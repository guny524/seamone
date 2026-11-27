"""Confine verified segment writes and queue insertion to the authorized review gate."""

import hashlib
import json
import re
import sqlite3
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

from check_lint_exceptions import git
from rust_tokens import tokens
from snapshot_baselines import errors as baseline_errors
from guard_integrity import base_commit

ROOT = Path(__file__).resolve().parent.parent
RUNTIME = ROOT / 'crates/seamone-runtime/src'
SNAPSHOT = 'scripts/review_writes.json'
GATE = 'WritePermit'
SQL_METHODS = {'execute', 'execute_batch', 'prepare', 'prepare_cached', 'prepare_with_flags', 'prepare_cached_with_flags', 'query_row', 'query_one', 'query_row_and_then'}


@dataclass
class Function:
    name: str
    start: int
    end: int
    header: list
    body: list
    testing: bool = False

    def digest(self):
        canonical = json.dumps([token.text for token in self.body], ensure_ascii=False)
        return hashlib.sha256(canonical.encode()).hexdigest()


def item_attributes(code, index):
    cursor = index - 1
    while cursor >= 0:
        if code[cursor].text in {'pub', 'async', 'unsafe', 'const', 'extern', 'default'}:
            cursor -= 1
        elif code[cursor].text == ')':
            start, depth = cursor - 1, 1
            while start >= 0 and depth:
                depth += (code[start].text == ')') - (code[start].text == '(')
                start -= 1
            if start < 0 or code[start].text != 'pub':
                break
            cursor = start - 1
        else:
            break
    found = []
    while cursor >= 0 and code[cursor].text == ']':
        end, depth = cursor, 1
        cursor -= 1
        while cursor >= 0 and depth:
            depth += (code[cursor].text == ']') - (code[cursor].text == '[')
            cursor -= 1
        if cursor < 0 or code[cursor].text != '#':
            break
        found.append(' '.join(token.text for token in code[cursor:end + 1]))
        cursor -= 1
    return found


def test_attribute(attributes):
    return '# [ cfg ( test ) ]' in attributes or '# [ test ]' in attributes


def test_files(root):
    found, seen = set(), set()
    def visit(path, inherited=False):
        if path in seen or not path.is_file():
            return
        seen.add(path)
        if inherited:
            found.add(path.relative_to(root).as_posix())
        code = list(tokens(path.read_text(encoding='utf-8')))
        directory = path.parent if path.name in {'lib.rs', 'mod.rs'} else path.parent / path.stem
        for index in range(len(code) - 2):
            if code[index].text != 'mod' or code[index + 2].text != ';':
                continue
            name = code[index + 1].text
            direct, nested = directory / (name + '.rs'), directory / name / 'mod.rs'
            child = direct if direct.is_file() else nested
            visit(child, inherited or test_attribute(item_attributes(code, index)))
    visit(root / 'crates/seamone-runtime/src/lib.rs')
    return found


def functions(source, test_only=False):
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
        parents = [name for start, finish, name, _ in containers if start < index < finish]
        testing = test_only or test_attribute(item_attributes(code, index)) or any(flag for start, finish, _, flag in containers if start < index < finish)
        name = '::'.join([*parents, label])
        if token.text == 'fn':
            found.append(Function(name, code[index].start, code[stop].end,
                                  code[index:end], code[end + 1:stop], testing))
        containers.append((end, stop, label, testing))
    return code, found


def test_regions(code):
    """Body ranges of test-only inline modules; their cfg attributes remain reviewed."""
    regions = []
    for index, token in enumerate(code):
        if token.text != 'mod' or not test_attribute(item_attributes(code, index)):
            continue
        end = index + 2
        if end < len(code) and code[end].text == '{':
            depth = 1
            end += 1
            while end < len(code) and depth:
                depth += (code[end].text == '{') - (code[end].text == '}')
                end += 1
            regions.append((token.start, code[end - 1].end))
    return regions


def persistence_digest(source):
    code, _ = functions(source)
    testing = test_regions(code)
    canonical = [token.text for token in code if not any(start <= token.start < stop for start, stop in testing)]
    return hashlib.sha256(json.dumps(canonical, ensure_ascii=False).encode()).hexdigest()


def string_value(token):
    value = token.text
    raw = re.fullmatch(r'r(#+)?"(.*)"\1', value, re.S)
    if raw:
        return raw[2]
    if value.startswith('r"') and value.endswith('"'):
        return value[2:-1]
    if not value.startswith('"'):
        return None
    result, index = [], 1
    escapes = {'n': '\n', 'r': '\r', 't': '\t', '0': '\0', '\\': '\\', '"': '"', "'": "'"}
    while index < len(value) - 1:
        character = value[index]
        index += 1
        if character != '\\':
            result.append(character)
            continue
        escape = value[index]
        index += 1
        if escape in escapes:
            result.append(escapes[escape])
        elif escape == 'x':
            result.append(chr(int(value[index:index + 2], 16)))
            index += 2
        elif escape == 'u' and value[index:index + 1] == '{':
            end = value.index('}', index)
            result.append(chr(int(value[index + 1:end].replace('_', ''), 16)))
            index = end + 1
        elif escape in '\r\n':
            while index < len(value) - 1 and value[index].isspace():
                index += 1
        else:
            raise ValueError(f'unsupported Rust escape: \\{escape}')
    return ''.join(result)


def sql_literal(token):
    value = token.text
    start = value.find('"')
    if start < 0 or not (value.startswith('"') or value.startswith('r')):
        return False
    rough = value[start + 1:]
    rough = re.sub(r'\A(?:\s|\\[nrt]|--[^\n]*(?:\n|$)|/\*.*?\*/)*', '', rough, flags=re.S)
    return bool(re.match(r'(?:SELECT|INSERT|REPLACE|UPDATE|DELETE|WITH|CREATE|ALTER|DROP|PRAGMA)\b', rough, re.I))


def database(migrations=None):

    db = sqlite3.connect(':memory:', cached_statements=0)
    # These optional/newer functions need names and arities for schema preparation and EXPLAIN,
    # not their runtime results. Override built-ins too so inspection uses the same definitions.
    db.create_function('unixepoch', -1, lambda *_: 0)
    db.create_function('json_extract', -1, lambda *_: None)
    if migrations is None:
        migrations = json.loads((ROOT / SNAPSHOT).read_text(encoding='utf-8'))['migrations']
    db.set_authorizer(lambda action, *_: sqlite3.SQLITE_DENY if action in {sqlite3.SQLITE_CREATE_TRIGGER, sqlite3.SQLITE_CREATE_TEMP_TRIGGER, sqlite3.SQLITE_CREATE_VIEW, sqlite3.SQLITE_CREATE_TEMP_VIEW} else sqlite3.SQLITE_OK)
    for migration in migrations:
        path = ROOT / migration['path']
        source = path.read_text(encoding='utf-8')
        if hashlib.sha256(source.encode()).hexdigest() != migration['sha256']:
            db.close()
            raise ValueError(f"{migration['path']}: migration content differs from the reviewed hash")
        db.executescript(source)
    db.set_authorizer(None)
    return db


def writes(db, sql):
    found = set()

    def authorize(action, table, column, _database, _trigger):
        table = (table or '').lower()
        if action in {sqlite3.SQLITE_CREATE_TRIGGER, sqlite3.SQLITE_CREATE_TEMP_TRIGGER, sqlite3.SQLITE_CREATE_VIEW, sqlite3.SQLITE_CREATE_TEMP_VIEW}:
            found.add('trigger/view definition')
        if action == sqlite3.SQLITE_INSERT and table in {'segments', 'training_queue'}:
            found.add(table)
        if action == sqlite3.SQLITE_UPDATE and (
            (table == 'segments' and (column or '').lower() == 'body')
            or (table == 'training_queue' and (column or '').lower() in {'segment_id', 'revision', 'recording_revision'})
        ):
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
            statement = re.sub(r'\A(?:\s|;|--[^\n]*(?:\n|$)|/\*.*?\*/)*', '', statement, flags=re.S)
            if not statement:
                continue
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


def prepared_calls(code):
    """Track local prepare* bindings and their lexical block, including let shadowing."""
    closing, stack = {}, []
    for index, token in enumerate(code):
        if token.text in {'(', '[', '{'}:
            stack.append(index)
        elif token.text in {')', ']', '}'} and stack:
            closing[stack.pop()] = index
    bindings, calls, data = [], set(), []
    for index, token in enumerate(code):
        if token.text == 'let':
            name = index + 1
            if name < len(code) and code[name].text == 'mut':
                name += 1
            if name + 1 < len(code) and code[name + 1].text == '=':
                end = name + 2
                while end < len(code) and code[end].text != ';':
                    end += 1
                stop = min((finish for start, finish in closing.items()
                            if code[start].text == '{' and start < index < finish), default=len(code))
                prepared = any(code[cursor].text in SQL_METHODS and code[cursor].text.startswith('prepare')
                               and cursor > name + 2 and code[cursor - 1].text in {'.', ':'}
                               and cursor + 1 < end and code[cursor + 1].text == '('
                               for cursor in range(name + 2, end))
                bindings.append((code[name].text, index, stop, prepared))
        if (token.text in {'execute', 'query_row'} and index >= 2
                and code[index - 1].text == '.' and index + 1 < len(code)
                and code[index + 1].text == '('):
            receiver = code[index - 2].text
            if index >= 3 and code[index - 3].text in {'.', ':'}:
                continue
            binding = next((item for item in reversed(bindings)
                            if item[0] == receiver and item[1] < index < item[2]), None)
            if binding is not None and binding[3]:
                calls.add(index)
                data.append((index + 1, closing.get(index + 1, index + 1)))
    return calls, data


def module_path_attribute(code, index):
    depth = 0
    for cursor in range(index - 1, -1, -1):
        text = code[cursor].text
        if text == ']':
            depth += 1
        elif text == '[':
            if depth:
                depth -= 1
            else:
                return cursor > 0 and code[cursor - 1].text == '#' and code[cursor + 1].text in {'path', 'cfg_attr'}
        elif not depth and text in {';', '{', '}'}:
            return False
    return False


def violations(source, path, policy, db, test_only=False):
    code, scopes = functions(source, test_only)
    reviewed_modules = {item['path']: item['sha256'] for item in policy.get('modules', [])}
    reviewed = path in reviewed_modules and persistence_digest(source) == reviewed_modules[path]
    testing = test_regions(code)
    connections = {'Connection'}
    for index, token in enumerate(code[:-2]):
        if token.text in connections and code[index + 1].text == 'as':
            connections.add(code[index + 2].text)
    changed = True
    while changed:
        previous = connections.copy()
        for index, token in enumerate(code[:-3]):
            if token.text == 'type' and code[index + 2].text == '=':
                end = index + 3
                while end < len(code) and code[end].text != ';':
                    end += 1
                if any(item.text in connections for item in code[index + 3:end]):
                    connections.add(code[index + 1].text)
        changed = connections != previous
    diagnostics = []
    decoded_arguments = {}
    prepared, bound_data = prepared_calls(code)

    def owner(token):
        enclosing = [scope for scope in scopes if scope.start <= token.start < scope.end]
        return min(enclosing, key=lambda scope: scope.end - scope.start) if enclosing else None

    def reject(token, reason):
        line = source.count('\n', 0, token.start) + 1
        diagnostics.append(f'{path}:{line}: {reason}; use {GATE} for verified writes and training_queue insertion')

    if path in reviewed_modules and not reviewed:
        diagnostics.append(f'{path}: reviewed persistence module changed; review its private types, authority and fixed SQL')

    for index, token in enumerate(code):
        scope = owner(token)
        name = scope.name if scope else '<outside function>'
        if test_only or (scope is not None and scope.testing) or any(start <= token.start < stop for start, stop in testing):
            continue
        receiver = index - 3
        connection_type = receiver >= 0 and code[receiver].text in connections
        if receiver >= 0 and code[receiver].text == '>':
            start, depth = receiver - 1, 1
            while start >= 0 and depth:
                depth += (code[start].text == '>') - (code[start].text == '<')
                start -= 1
            connection_type = any(item.text in connections for item in code[start + 1:receiver])
        is_open = (token.text.startswith('open') and index >= 3
                   and [item.text for item in code[index - 2:index]] == [':', ':']
                   and connection_type)
        authority_owner = path == 'crates/seamone-runtime/src/write_guard.rs' or (path == 'crates/seamone-runtime/src/store.rs' and name == 'Store::open')
        if token.text in {'Batch', 'raw_execute', 'raw_bind_parameter'} and not reviewed:
            reject(token, f'{name} cannot use raw SQL API {token.text} outside the reviewed persistence module')
        if token.text in {'authorizer', 'hooks'} and not authority_owner:
            reject(token, f'{name} cannot change SQLite authorization outside Store::open and write_guard')
        if is_open and (path != 'crates/seamone-runtime/src/store.rs' or name != 'Store::open'):
            reject(token, f'{name} cannot open a SQLite connection outside Store::open')
        if token.text == 'path' and index + 1 < len(code) and code[index + 1].text == '=' and module_path_attribute(code, index):
            target = index + 1
            while target < len(code) and code[target].text not in {'mod', 'fn', 'struct', 'enum', 'impl'}:
                target += 1
            if not test_attribute(item_attributes(code, target)):
                reject(token, f'{name} cannot use #[path] outside test-only code')
        if token.text in {'stringify', 'include_str'} and index + 1 < len(code) and code[index + 1].text == '!':
            migration = (token.text == 'include_str' and name == 'Store::open'
                         and path == 'crates/seamone-runtime/src/store.rs'
                         and index + 3 < len(code)
                         and string_value(code[index + 3]) in {Path(item['path']).name for item in policy.get('migrations', [])})
            if not migration:
                reject(token, f'{name} cannot use {token.text}! outside reviewed migrations')
        if token.text == '.' and index + 1 < len(code) and code[index + 1].text == '$':
            reject(token, f'{name} cannot dispatch a method through a macro variable')
        if (token.text in SQL_METHODS and index > 0 and code[index - 1].text in {'.', ':'}
                and index + 1 < len(code) and code[index + 1].text != '('):
            reject(token, f'{name} cannot use a SQL method reference')
        if index in prepared:
            continue

        if (token.text in SQL_METHODS and index > 0 and code[index - 1].text in {'.', ':'}
                and index + 2 < len(code) and code[index + 1].text == '('):
            argument_index = index + 2
            if code[index - 1].text == ':':
                depth = 0
                while argument_index < len(code):
                    text = code[argument_index].text
                    if depth == 0 and text == ',':
                        argument_index += 1
                        break
                    depth += text in {'(', '[', '{'}
                    depth -= text in {')', ']', '}'}
                    argument_index += 1
            if argument_index >= len(code):
                reject(token, f'{name} has an unreadable SQL argument')
                continue
            argument = code[argument_index]
            migration = (path == 'crates/seamone-runtime/src/store.rs' and name == 'Store::open'
                         and [item.text for item in code[index + 2:index + 5]] == ['include_str', '!', '(']
                         and index + 5 < len(code) and string_value(code[index + 5])
                         in {Path(item['path']).name for item in policy.get('migrations', [])})
            try:
                argument_sql = string_value(argument)
            except (ValueError, IndexError) as error:
                reject(argument, f'{name} has an invalid Rust SQL literal: {error}')
                continue
            if argument_sql is None and not migration:
                reject(token, f'{name} uses dynamic SQL that cannot establish a verified-write boundary')
            elif argument_sql is not None:
                decoded_arguments[argument.start] = argument_sql
        if token.start not in decoded_arguments and (any(start < index < stop for start, stop in bound_data) or not sql_literal(token)):
            continue
        try:
            sql = decoded_arguments.get(token.start) if token.start in decoded_arguments else string_value(token)
        except (ValueError, IndexError) as error:
            reject(token, f'{name} has an invalid Rust SQL literal: {error}')
            continue
        if sql is None:
            continue
        try:
            critical = writes(db, sql)
        except sqlite3.Error as error:
            reject(token, f'{name} has SQL that cannot be inspected: {error}')
            continue
        if not critical:
            continue
        if 'trigger/view definition' in critical:
            reject(token, f'{name} defines a trigger or view that can hide verified writes')
            continue
        if reviewed:
            continue
        reject(token, f'{name} writes {", ".join(sorted(critical))} outside the authorized review gate')
    return diagnostics


def main():
    try:
        base_commit(ROOT, os.environ.get('LINT_BASE_REF', 'origin/develop'))
    except (ValueError, subprocess.CalledProcessError) as error:
        print(str(error), file=sys.stderr)
        return 1
    diagnostics = baseline_errors(ROOT, SNAPSHOT)
    try:
        policy = json.loads((ROOT / SNAPSHOT).read_text(encoding='utf-8'))
        names = set(filter(None, git(ROOT, 'ls-files', '--cached', '--others', '--exclude-standard', '-z', '--',
                                    'crates/seamone-runtime/src').split('\0')))
        db = database()
        tests = test_files(ROOT)
        try:
            for name in sorted(names):
                path = ROOT / name
                if path.suffix == '.rs' and path.is_file():
                    diagnostics.extend(violations(path.read_text(encoding='utf-8'), name, policy, db, name in tests))
        finally:
            db.close()
    except (OSError, ValueError, KeyError, TypeError, sqlite3.Error) as error:
        diagnostics.append(f'{SNAPSHOT}: cannot inspect review writes: {error}; use {GATE}')
    for diagnostic in diagnostics:
        print(diagnostic, file=sys.stderr)
    return int(bool(diagnostics))


if __name__ == '__main__':
    sys.exit(main())
