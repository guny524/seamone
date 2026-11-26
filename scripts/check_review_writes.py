"""Confine verified segment writes and queue insertion to the authorized review gate."""

import hashlib
import json
import re
import sqlite3
import sys
from dataclasses import dataclass
from pathlib import Path

from check_lint_exceptions import git
from rust_tokens import tokens
from snapshot_baselines import errors as baseline_errors

ROOT = Path(__file__).resolve().parent.parent
RUNTIME = ROOT / 'crates/seamone-runtime/src'
SNAPSHOT = 'scripts/review_writes.json'
GATE = 'Store::mutate(AuthorizedReview)'
SQL_METHODS = {'execute', 'execute_batch', 'prepare', 'prepare_cached', 'query_row'}


@dataclass
class Function:
    name: str
    start: int
    end: int
    header: list
    body: list

    def digest(self):
        canonical = json.dumps([token.text for token in self.body], ensure_ascii=False)
        return hashlib.sha256(canonical.encode()).hexdigest()


def functions(source):
    code = list(tokens(source))
    closing, stack = {}, []
    for index, token in enumerate(code):
        if token.text == '{':
            stack.append(index)
        elif token.text == '}':
            if not stack:
                raise ValueError('unbalanced Rust braces')
            closing[stack.pop()] = index
    if stack:
        raise ValueError('unbalanced Rust braces')
    containers, found = [], []
    for index, token in enumerate(code):
        if token.text not in {'impl', 'mod', 'fn'} or index + 1 >= len(code):
            continue
        if token.text == 'fn' and not re.fullmatch(r'[A-Za-z_]\w*', code[index + 1].text):
            continue
        end = index + 1
        while end < len(code) and code[end].text not in {'{', ';'}:
            end += 1
        if end >= len(code) or code[end].text != '{':
            continue
        stop = closing[end]
        if token.text == 'impl':
            label = ' '.join(item.text for item in code[index + 1:end])
        else:
            label = code[index + 1].text
        parents = [name for start, finish, name in containers if start < index < finish]
        name = '::'.join([*parents, label])
        if token.text == 'fn':
            found.append(Function(name, code[index].start, code[stop].end,
                                  code[index:end], code[end + 1:stop]))
        containers.append((end, stop, label))
    return code, found


def string_value(token):
    value = token.text
    raw = re.fullmatch(r'r(#+)?"(.*)"\1', value, re.S)
    if raw:
        return raw[2]
    if value.startswith('r"') and value.endswith('"'):
        return value[2:-1]
    if value.startswith('"'):
        inner = re.sub(r'\\\n\s*', '', value[1:-1])
        inner = inner.replace('\n', r'\n').replace('\r', r'\r').replace('\t', r'\t')
        return json.loads('"' + inner + '"')
    return None


def database():
    # Only an empty in-memory database is touched. EXPLAIN compiles SQL without writing rows.
    db = sqlite3.connect(':memory:', cached_statements=0)
    for name in ('schema.sql', 'schema-v2.sql', 'schema-v3.sql', 'schema-v4.sql'):
        db.executescript((RUNTIME / name).read_text(encoding='utf-8'))
    return db


def writes(db, sql):
    found = set()

    def authorize(action, table, column, _database, _trigger):
        table = (table or '').lower()
        if action == sqlite3.SQLITE_INSERT and table in {'segments', 'training_queue'}:
            found.add(table)
        if action == sqlite3.SQLITE_UPDATE and table == 'segments' and (column or '').lower() == 'body':
            found.add(table)
        return sqlite3.SQLITE_OK

    db.set_authorizer(authorize)
    try:
        partial = ''
        statements = []
        for char in sql:
            partial += char
            if char == ';' and sqlite3.complete_statement(partial):
                statements.append(partial)
                partial = ''
        if partial.strip():
            statements.append(partial)
        for statement in statements:
            try:
                db.execute('EXPLAIN ' + statement)
            except sqlite3.ProgrammingError as error:
                # Authorization runs during preparation, before parameter binding.
                if 'binding' not in str(error).lower():
                    raise
            except sqlite3.OperationalError as error:
                # EXPLAIN never creates fixture triggers, so their later teardown has no live target.
                if not (re.match(r'\s*DROP\s+TRIGGER\b', statement, re.I) and 'no such trigger:' in str(error)):
                    raise
    finally:
        db.set_authorizer(None)
    return found


def violations(source, path, policy, db):
    code, scopes = functions(source)
    authorized = {item['function'] for item in policy['authorized'] if item['path'] == path}
    unverified = {item['function']: item['body_sha256'] for item in policy['unverified'] if item['path'] == path}
    diagnostics = []

    def owner(token):
        enclosing = [scope for scope in scopes if scope.start <= token.start < scope.end]
        return min(enclosing, key=lambda scope: scope.end - scope.start) if enclosing else None

    def reject(token, reason):
        line = source.count('\n', 0, token.start) + 1
        diagnostics.append(f'{path}:{line}: {reason}; use {GATE} for verified writes and training_queue insertion')

    for name in authorized | unverified.keys():
        matches = [scope for scope in scopes if scope.name == name]
        if len(matches) != 1:
            diagnostics.append(f'{path}: define exactly one reviewed writer {name}; use {GATE}')
            continue
        scope = matches[0]
        if name in authorized and not re.search(
            r'\bauthorization : crate : : playback_evidence : : AuthorizedReview <',
            ' '.join(token.text for token in scope.header)
        ):
            reject(scope.header[0], f'{name} must take AuthorizedReview by value')
        if name in unverified and scope.digest() != unverified[name]:
            reject(scope.header[0], f'{name} changed its reviewed unverified-only body')

    for index, token in enumerate(code):
        scope = owner(token)
        name = scope.name if scope else '<outside function>'
        # Dynamic SQL expressions must not hide writes from the literal inspection.
        if (token.text in SQL_METHODS and index > 0 and code[index - 1].text == '.'
                and index + 2 < len(code) and code[index + 1].text == '('):
            argument = code[index + 2]
            migration = (path == 'crates/seamone-runtime/src/store.rs' and name == 'Store::open'
                         and [item.text for item in code[index + 2:index + 5]] == ['include_str', '!', '(']
                         and index + 5 < len(code) and string_value(code[index + 5])
                         in {'schema.sql', 'schema-v2.sql', 'schema-v3.sql', 'schema-v4.sql'})
            if string_value(argument) is None and not migration:
                reject(token, f'{name} uses dynamic SQL that cannot establish a verified-write boundary')
        sql = string_value(token)
        if sql is None:
            continue
        leading = re.sub(r'\A(?:\s|--[^\n]*(?:\n|$)|/\*.*?\*/)*', '', sql, flags=re.S)
        if not re.match(r'(?:SELECT|INSERT|REPLACE|UPDATE|DELETE|WITH|CREATE|ALTER|DROP|PRAGMA)\b', leading, re.I):
            continue
        try:
            critical = writes(db, sql)
        except sqlite3.Error as error:
            reject(token, f'{name} has SQL that cannot be inspected: {error}')
            continue
        if not critical:
            continue
        if name in authorized:
            continue
        if critical == {'segments'} and name in unverified and scope.digest() == unverified[name]:
            continue
        reject(token, f'{name} writes {", ".join(sorted(critical))} outside the authorized review gate')
    return diagnostics


def main():
    diagnostics = baseline_errors(ROOT, SNAPSHOT)
    try:
        policy = json.loads((ROOT / SNAPSHOT).read_text(encoding='utf-8'))
        names = set(filter(None, git(ROOT, 'ls-files', '--cached', '--others', '--exclude-standard', '-z', '--',
                                    'crates/seamone-runtime/src').split('\0')))
        db = database()
        try:
            for name in sorted(names):
                path = ROOT / name
                if path.suffix == '.rs' and path.is_file():
                    diagnostics.extend(violations(path.read_text(encoding='utf-8'), name, policy, db))
        finally:
            db.close()
    except (OSError, ValueError, KeyError, TypeError, sqlite3.Error) as error:
        diagnostics.append(f'{SNAPSHOT}: cannot inspect review writes: {error}; use {GATE}')
    for diagnostic in diagnostics:
        print(diagnostic, file=sys.stderr)
    return int(bool(diagnostics))


if __name__ == '__main__':
    sys.exit(main())
