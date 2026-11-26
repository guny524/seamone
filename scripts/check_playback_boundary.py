"""Freeze the evidence module's callable surface and trait constructors."""

import collections
import json
import sys
from pathlib import Path

from check_runtime_api import surface
from rust_tokens import tokens

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / 'crates/seamone-runtime/src/playback_evidence.rs'
SNAPSHOT = ROOT / 'scripts/playback_api.json'


def declarations(source):
    code = list(tokens(source))
    found = []
    public = iter(surface(source, restricted=True))
    for index, token in enumerate(code):
        if token.text != 'pub':
            continue
        start = index
        while start > 0 and code[start - 1].text == ']':
            bracket, depth = start - 2, 1
            while bracket >= 0 and depth:
                depth += (code[bracket].text == ']') - (code[bracket].text == '[')
                bracket -= 1
            if bracket < 0 or code[bracket].text != '#':
                break
            start = bracket
        attributes = ' '.join(item.text for item in code[start:index])
        found.append((attributes + ' ' if attributes else '') + next(public))
    for index, token in enumerate(code):
        if token.text == 'impl':
            end = index + 1
            while end < len(code) and code[end].text != '{':
                end += 1
            header = [item.text for item in code[index:end]]
            if 'for' in header:
                found.append(' '.join(header))
        if token.text == '#' and index + 2 < len(code) and code[index + 1].text == '[':
            end, depth = index + 2, 1
            while end < len(code) and depth:
                depth += (code[end].text == '[') - (code[end].text == ']')
                end += 1
            attribute = code[index:end]
            if any(item.text == 'derive' for item in attribute):
                found.append(' '.join(item.text for item in attribute))
    return sorted(found)


def errors(actual, expected):
    current, reviewed = map(collections.Counter, (actual, expected))
    found = []
    for declaration in current - reviewed:
        found.append(f'playback_evidence.rs: unreviewed evidence declaration: {declaration}; '
                     'issue tokens only through Playbacks::issue with current audio, '
                     'review scripts/playback_api.json before changing this boundary')
    if reviewed - current:
        found.append('scripts/playback_api.json: an evidence declaration changed or disappeared; review the callers')
    return found


def main():
    try:
        diagnostics = errors(declarations(SOURCE.read_text(encoding='utf-8')),
                             json.loads(SNAPSHOT.read_text(encoding='utf-8'))['declarations'])
    except (OSError, ValueError, KeyError, TypeError) as error:
        diagnostics = [f'{SNAPSHOT}: cannot read the evidence boundary: {error}']
    for diagnostic in diagnostics:
        print(diagnostic, file=sys.stderr)
    return int(bool(diagnostics))


if __name__ == '__main__':
    sys.exit(main())
