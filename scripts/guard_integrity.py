"""Shared local base validation and readers for the actual inline CI programs."""

import os
import subprocess
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def git(root, *args):
    return subprocess.check_output(['git', *args], cwd=root, stderr=subprocess.PIPE).decode()


def base_commit(root, ref, *, ci=None):
    commit = git(root, 'rev-parse', '--verify', '--end-of-options', ref + '^{commit}').strip()
    ci = os.environ.get('CI') == 'true' if ci is None else ci
    if ci and commit == git(root, 'rev-parse', 'HEAD').strip():
        raise ValueError('guard base must differ from HEAD in CI')
    return commit


def workflow_program(file, step):
    """Read the named YAML literal block, so tests execute the CI source itself."""
    lines = (ROOT / file).read_text().splitlines()
    start = next(index for index, line in enumerate(lines) if line.strip() == '- name: ' + step)
    start = next(index for index in range(start + 1, len(lines)) if lines[index].strip() == 'run: |') + 1
    indent = len(lines[start]) - len(lines[start].lstrip())
    end = start
    while end < len(lines) and (not lines[end].strip() or lines[end].startswith(' ' * indent)):
        end += 1
    block = textwrap.dedent('\n'.join(lines[start:end]))
    return block.split('\n', 1)[1].rsplit('\nPY', 1)[0]
