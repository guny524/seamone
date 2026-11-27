"""Require reviewed PR-base snapshots, never accept self-approved additions."""

import collections
import json
import os
import subprocess

from check_lint_exceptions import git

REVIEW = '사람이 확인한 뒤에만 늘립니다. 이 PR의 CI는 사람이 보고 판단합니다.'
SNAPSHOTS = {
    'scripts/comments.json': ('comments',),
    'scripts/runtime_api.json': ('public',),
    'scripts/playback_api.json': ('declarations', 'callers'),
    'scripts/unsafe_files.json': ('functions',),
    'scripts/review_writes.json': ('modules', 'migrations'),
}


def entries(data, key):
    value = data[key]
    if not isinstance(value, list):
        raise ValueError(f'{key} must be a list')
    return collections.Counter(json.dumps(item, sort_keys=True, ensure_ascii=False) for item in value)


def errors(root, path, ref=None):
    ref = ref or os.environ.get('LINT_BASE_REF', 'origin/develop')
    try:
        commit = git(root, 'rev-parse', '--verify', '--end-of-options', ref + '^{commit}').strip()
        previous = json.loads(git(root, 'show', f'{commit}:{path}'))
        current = json.loads((root / path).read_text(encoding='utf-8'))
        found = []
        for key in SNAPSHOTS[path]:
            for item, count in sorted((entries(current, key) - entries(previous, key)).items()):
                found.append(f'{path} vs {ref}: {count} unreviewed snapshot addition(s) in {key}: {item}; {REVIEW}')
        return found
    except (OSError, subprocess.CalledProcessError, ValueError, KeyError, TypeError) as error:
        return [f'{path}: cannot read reviewed snapshot at {ref}: {error}; '
                f'base에 같은 목록 파일이 있어야 합니다. {REVIEW}']
