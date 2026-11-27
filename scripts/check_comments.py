"""Freeze only comments explaining external constraints; express our behavior in code."""

import collections
import hashlib
import io
import json
import os
import re
import subprocess
import sys
import tokenize
from pathlib import Path

from check_lint_exceptions import git
from guard_integrity import base_commit
from rust_tokens import tokens
from snapshot_baselines import errors as baseline_errors

ROOT = Path(__file__).resolve().parent.parent
SNAPSHOT = 'scripts/comments.json'
FIX = '이름과 구조로 드러내거나 검사로 옮기세요'


def rust_comments(source):
    for token in tokens(source, include_comments=True):
        text = token.text
        if text.startswith('//'):
            if text.startswith('//!') or (text.startswith('///') and not text.startswith('////')):
                continue
        elif text.startswith('/*'):
            if text.startswith('/*!') or (text.startswith('/**') and not text.startswith('/***')):
                continue
        else:
            continue
        yield source.count('\n', 0, token.start) + 1, text


def python_comments(source):
    for token in tokenize.generate_tokens(io.StringIO(source).readline):
        if token.type != tokenize.COMMENT:
            continue
        if token.start[0] == 1 and token.string.startswith('#!'):
            continue
        if token.start[0] <= 2 and re.match(r'#.*?coding[:=]\s*[-\w.]+', token.string):
            continue
        yield token.start[0], token.string


def inventory(root):
    names = git(root, 'ls-files', '--cached', '--others', '--exclude-standard', '-z').split('\0')
    found = []
    for name in sorted(set(filter(None, names))):
        path = root / name
        if not path.is_file() or path.suffix not in {'.rs', '.py'}:
            continue
        source = path.read_text(encoding='utf-8')
        comments = rust_comments(source) if path.suffix == '.rs' else python_comments(source)
        for line, comment in comments:
            found.append((name, line, hashlib.sha256(comment.encode()).hexdigest(), comment))
    return found


def violations(root, entries):
    expected = collections.Counter((entry['path'], entry['sha256']) for entry in entries)
    actual = collections.Counter()
    errors = []
    for path, line, digest, _ in inventory(root):
        actual[path, digest] += 1
        if actual[path, digest] > expected[path, digest]:
            errors.append(f'{path}:{line}: unreviewed comment; {FIX}')
    for (path, _), count in (expected - actual).items():
        errors.append(f'{path}: {count} stale comment snapshot entry(s); remove the missing entries')
    return errors


def main():
    try:
        ref = os.environ.get('LINT_BASE_REF', 'origin/develop')
        base_commit(ROOT, ref)
        entries = json.loads((ROOT / SNAPSHOT).read_text(encoding='utf-8'))['comments']
        errors = baseline_errors(ROOT, SNAPSHOT, ref) + violations(ROOT, entries)
    except (OSError, ValueError, KeyError, TypeError, tokenize.TokenError,
            subprocess.CalledProcessError) as error:
        errors = [f'{SNAPSHOT}: cannot check comments: {error}; {FIX}']
    for error in errors:
        print(error, file=sys.stderr)
    return int(bool(errors))


if __name__ == '__main__':
    sys.exit(main())
