"""Compare the runtime's public declarations with its reviewed API snapshot."""

import collections
import json
import sys
from pathlib import Path

from rust_tokens import tokens

ROOT = Path(__file__).resolve().parent.parent
SNAPSHOT = Path(__file__).with_name("runtime_api.json")
RUNTIME = ROOT / "crates/seamone-runtime/src"


def surface(source, *, restricted=False):
    code = list(tokens(source))
    for index, token in enumerate(code):
        if token.text != "pub" or index + 1 >= len(code):
            continue
        declaration_start = index + 1
        if code[declaration_start].text == "(":
            if not restricted:
                continue
            while declaration_start < len(code) and code[declaration_start].text != ")":
                declaration_start += 1
            declaration_start += 1
        if declaration_start >= len(code):
            raise ValueError("incomplete Rust visibility")
        kind = code[declaration_start].text
        is_use = kind == "use"
        end, stack = declaration_start, []
        while end < len(code):
            text = code[end].text
            if not stack and (text == ";" or (text == "{" and not is_use) or text in (",", "}")):
                if text == "{" and kind in ("enum", "trait"):
                    # Variants and trait members are public without a `pub` token.
                    depth = 1
                    end += 1
                    while end < len(code) and depth:
                        depth += (code[end].text == "{") - (code[end].text == "}")
                        end += 1
                break
            if text in ("(", "[", "{", "<"):
                stack.append(text)
            elif stack and text == {"(": ")", "[": "]", "{": "}", "<": ">"}[stack[-1]]:
                if not (text == ">" and end and code[end - 1].text == "-"):
                    stack.pop()
            end += 1
        declaration = " ".join(item.text for item in code[index:end])
        yield declaration + (" {" if kind == "mod" and end < len(code) and code[end].text == "{" else "")


def inventory():
    pending = [RUNTIME / "lib.rs"]
    found, seen = [], set()
    while pending:
        path = pending.pop()
        if path in seen:
            continue
        seen.add(path)
        directory = path.parent if path.name in ("lib.rs", "mod.rs") else path.parent / path.stem
        source = path.read_text(encoding="utf-8")
        for declaration in surface(source):
            found.append([path.relative_to(ROOT).as_posix(), declaration])
            parts = declaration.split()
            if parts[:2] == ["pub", "mod"] and len(parts) == 3:
                direct = directory / (parts[2] + ".rs")
                nested = directory / parts[2] / "mod.rs"
                if direct.is_file():
                    pending.append(direct)
                elif nested.is_file():
                    pending.append(nested)
                else:
                    raise ValueError(f"{path}: public module {parts[2]} must have a readable source")
        # The source of a re-export is also part of the reviewed boundary.
        if path.name == "lib.rs":
            code = list(tokens(source))
            for index in range(len(code) - 4):
                if [item.text for item in code[index:index + 2]] != ["pub", "use"]:
                    continue
                module = code[index + 2].text
                target = path.parent / (module + ".rs")
                if not target.is_file():
                    continue
                # Conservatively freeze helper declarations in that source too.
                pending.append(target)
    return sorted(found)


def main():
    expected = json.loads(SNAPSHOT.read_text(encoding="utf-8"))["public"]
    try:
        actual = inventory()
    except (OSError, ValueError) as error:
        print(f"{error}; before updating {SNAPSHOT}, check LibraryService's evidence checks", file=sys.stderr)
        return 1
    actual = collections.Counter(map(tuple, actual))
    expected = collections.Counter(map(tuple, expected))
    if actual == expected:
        return 0
    for file, declaration in actual - expected:
        print(f"{file}: unreviewed public item: {declaration}; before adding it to "
              f"{SNAPSHOT}, check that it uses LibraryService's evidence checks", file=sys.stderr)
    if expected - actual:
        print(f"{SNAPSHOT}: a public declaration changed or disappeared; review the callers", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
