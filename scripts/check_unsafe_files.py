"""Confine Rust unsafe to existing adapters and freeze that scope against the base."""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

from check_lint_exceptions import git
from snapshot_baselines import errors as baseline_errors
from rust_tokens import tokens

ROOT = Path(__file__).resolve().parent.parent
BASELINE = 'scripts/unsafe_files.json'


def unsafe_lines(source):
    for token in tokens(source):
        if token.text == 'unsafe':
            yield source.count('\n', 0, token.start) + 1


def allowed_files(source):
    allowed = json.loads(source)['files']
    if not isinstance(allowed, list) or any(
        not isinstance(name, str) or Path(name).is_absolute()
        or '..' in Path(name).parts or not name.endswith('.rs') for name in allowed
    ) or len(set(allowed)) != len(allowed):
        raise ValueError('files must be unique relative Rust source paths')
    return set(allowed)


def check(root, ref):
    try:
        diagnostics = baseline_errors(root, BASELINE, ref)
        allowed = allowed_files((root / BASELINE).read_text(encoding='utf-8'))
        names = set(filter(None, git(root, 'ls-files', '--cached', '--others', '--exclude-standard', '-z').split('\0')))
        for name in sorted(names):
            path = root / name
            if path.suffix != '.rs' or not path.is_file():
                continue
            lines = list(unsafe_lines(path.read_text(encoding='utf-8')))
            if lines and name not in allowed:
                diagnostics.extend(f'{name}:{line}: unsafe is forbidden here; use the existing native adapter, '
                                   'do not forge playback authorization' for line in lines)
        return diagnostics
    except (OSError, subprocess.CalledProcessError, ValueError, KeyError, TypeError) as error:
        return diagnostics + [f'{BASELINE}: cannot verify unsafe boundary at {ref}: {error}; fetch a readable base ref']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-ref', default=os.environ.get('LINT_BASE_REF', 'origin/develop'))
    args = parser.parse_args()
    diagnostics = check(ROOT, args.base_ref)
    for diagnostic in diagnostics:
        print(diagnostic, file=sys.stderr)
    return int(bool(diagnostics))


if __name__ == '__main__':
    sys.exit(main())
