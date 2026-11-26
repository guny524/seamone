"""Confine Rust unsafe to existing adapters and freeze that scope against the base."""

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

from check_lint_exceptions import git
from snapshot_baselines import errors as baseline_errors
from guard_integrity import base_commit
from rust_tokens import tokens
from check_review_writes import functions

ROOT = Path(__file__).resolve().parent.parent
BASELINE = 'scripts/unsafe_files.json'


def unsafe_lines(source):
    for token in tokens(source):
        if token.text == 'unsafe':
            yield source.count('\n', 0, token.start) + 1


def check(root, ref):
    diagnostics = baseline_errors(root, BASELINE, ref)
    try:
        reviewed = json.loads((root / BASELINE).read_text(encoding='utf-8'))['functions']
        names = set(filter(None, git(root, 'ls-files', '--cached', '--others', '--exclude-standard', '-z').split('\0')))
        for name in sorted(names):
            path = root / name
            if path.suffix != '.rs' or not path.is_file():
                continue
            source = path.read_text(encoding='utf-8')
            code, scopes = functions(source)
            allowed = []
            for entry in reviewed:
                if entry['path'] != name:
                    continue
                matches = [scope for scope in scopes if scope.name == entry['function']]
                if len(matches) == 1 and matches[0].digest() == entry['body_sha256']:
                    allowed.append(matches[0])
                else:
                    diagnostics.append(f'{name}: reviewed unsafe function {entry["function"]} changed; human base review is required')
            for index, token in enumerate(code):
                line = source.count('\n', 0, token.start) + 1
                if token.text == 'unsafe' and not any(scope.start <= token.start < scope.end for scope in allowed):
                    diagnostics.append(f'{name}:{line}: unsafe is forbidden here; keep it in the exact reviewed capture function')
                if name.startswith('crates/seamone-runtime/src/') and token.text == 'include' and index + 1 < len(code) and code[index + 1].text == '!':
                    diagnostics.append(f'{name}:{line}: runtime include! is forbidden; keep Rust code in inspected modules')
                if name == 'crates/seamone-runtime/src/capture.rs' and (
                    token.text == 'playback_evidence' or token.text.startswith('transmute')
                    or token.text == 'Store'
                    or (token.text == 'store' and (index == 0 or code[index - 1].text != '.'))
                ):
                    diagnostics.append(f'{name}:{line}: capture cannot access store/playback evidence or transmute values')
        return diagnostics
    except (OSError, subprocess.CalledProcessError, ValueError, KeyError, TypeError) as error:
        return diagnostics + [f'{BASELINE}: cannot verify unsafe boundary at {ref}: {error}; fetch a readable base ref']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base-ref', default=os.environ.get('LINT_BASE_REF', 'origin/develop'))
    args = parser.parse_args()
    try:
        base_commit(ROOT, args.base_ref)
    except (ValueError, subprocess.CalledProcessError) as error:
        print(str(error), file=sys.stderr)
        return 1
    diagnostics = check(ROOT, args.base_ref)
    for diagnostic in diagnostics:
        print(diagnostic, file=sys.stderr)
    return int(bool(diagnostics))


if __name__ == '__main__':
    sys.exit(main())
