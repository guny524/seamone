"""Run base-commit guards against head sources and flag changes requiring human review."""

import argparse
import json
import os
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REVIEW = '검사와 규칙 파일이 바뀌었다, 사람이 확인한 뒤 merge 한다'


def git(root, *args):
    return subprocess.check_output(['git', *args], cwd=root, stderr=subprocess.PIPE).decode()


def base_commit(root, ref):
    commit = git(root, 'rev-parse', '--verify', '--end-of-options', ref + '^{commit}').strip()
    if commit == git(root, 'rev-parse', 'HEAD').strip():
        raise ValueError('guard base must differ from HEAD; never compare a snapshot with itself')
    return commit


def resolve_base(root, event, ref=None):
    selected = ref or event.get('pull_request', {}).get('base', {}).get('sha') or event.get('before')
    if selected and set(selected) == {'0'}:
        selected = git(root, 'merge-base', 'HEAD', 'origin/develop').strip()
    return base_commit(root, selected or os.environ.get('LINT_BASE_REF', 'origin/develop'))


def protected_changes(root, base):
    names = sorted(set(git(root, 'diff', '--name-only', '--no-renames', base, '--').splitlines())
                   | set(git(root, 'ls-files', '--others', '--exclude-standard').splitlines()))
    protected = [name for name in names if name.startswith(('scripts/', '.github/'))
                 or name in {'Makefile', 'research/Makefile', 'AGENTS.md'}]
    if 'Cargo.toml' in names:
        old = tomllib.loads(git(root, 'show', f'{base}:Cargo.toml')).get('workspace', {}).get('lints')
        new = tomllib.loads((root / 'Cargo.toml').read_text()).get('workspace', {}).get('lints')
        if old != new:
            protected.append('Cargo.toml [workspace.lints]')
    return protected


def run_base_guards(root, base):
    base = base_commit(root, base)
    workflow = json.loads((ROOT / '.github/workflows/guard-integrity.yml').read_text())
    step = workflow['jobs']['base-guards']['steps'][-1]['run']
    program = step.split("\n", 1)[1].rsplit("\nPY", 1)[0]
    environment = os.environ.copy()
    environment.update(LINT_BASE_REF=base, SOURCE_ROOT=str(root))
    return subprocess.run([sys.executable, '-I', '-c', program], env=environment).returncode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('base', 'review', 'frozen'))
    parser.add_argument('--base-ref')
    args = parser.parse_args()
    try:
        event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text()) if os.environ.get('GITHUB_EVENT_PATH') else {}
        base = resolve_base(ROOT, event, args.base_ref)
        if args.mode == 'base':
            print(base)
            if os.environ.get('GITHUB_OUTPUT'):
                with Path(os.environ['GITHUB_OUTPUT']).open('a') as output:
                    output.write(f'commit={base}\n')
            return 0
        if args.mode == 'frozen':
            return run_base_guards(ROOT, base)
        changes = protected_changes(ROOT, base)
        for path in changes:
            print(f'{path}: {REVIEW}', file=sys.stderr)
        return int(bool(changes))
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f'cannot verify guard integrity: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
