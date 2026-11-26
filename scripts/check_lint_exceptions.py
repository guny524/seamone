"""Reject additions to the existing, still unapproved lint suppressions."""

import collections
import io
import json
import re
import subprocess
import sys
import tokenize
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASELINE = Path(__file__).with_name("lint_exceptions.json")


def rust_exceptions(source):
    for start in re.finditer(r"^[ \t]*#!?\[", source, re.MULTILINE):
        depth, quoted, escaped = 1, False, False
        end = start.end()
        while end < len(source) and depth:
            char = source[end]
            if escaped:
                escaped = False
            elif char == "\\" and quoted:
                escaped = True
            elif char == '"':
                quoted = not quoted
            elif not quoted:
                depth += (char == "[") - (char == "]")
            end += 1
        attribute = source[start.start():end]
        if re.search(r"\b(?:allow|expect)\s*\(", attribute):
            yield re.sub(r"\s+", " ", attribute).strip()


def python_exceptions(source):
    for token in tokenize.generate_tokens(io.StringIO(source).readline):
        if token.type == tokenize.COMMENT and re.search(
            r"\bnoqa\b|\btype\s*:\s*ignore\b", token.string, re.IGNORECASE
        ):
            yield token.string.strip()


def configuration_exceptions(path, source):
    data = tomllib.loads(source)
    if path.name == "Cargo.toml":
        groups = [data.get("lints", {}), data.get("workspace", {}).get("lints", {})]
        for group in groups:
            for tool, rules in group.items():
                if tool == "workspace":
                    continue
                for rule, value in rules.items():
                    level = value.get("level") if isinstance(value, dict) else value
                    if level == "allow":
                        yield f"{tool}.{rule}=allow"
    else:
        lint = data.get("tool", {}).get("ruff", {}).get("lint", {})
        for key in ("ignore", "extend-ignore", "per-file-ignores", "extend-per-file-ignores"):
            value = lint.get(key)
            if value:
                yield f"ruff.{key}={json.dumps(value, sort_keys=True)}"


def inventory(root, paths):
    found = []
    for path in paths:
        if not path.is_file():
            continue
        if path.suffix == ".rs":
            entries = rust_exceptions(path.read_text(encoding="utf-8"))
        elif path.suffix == ".py":
            entries = python_exceptions(path.read_text(encoding="utf-8"))
        elif path.name in ("Cargo.toml", "pyproject.toml"):
            entries = configuration_exceptions(path, path.read_text(encoding="utf-8"))
        else:
            continue
        found.extend((path.relative_to(root).as_posix(), entry) for entry in entries)
    return found


def additions(actual, baseline):
    return collections.Counter(map(tuple, actual)) - collections.Counter(map(tuple, baseline))


def main():
    output = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
    )
    paths = sorted({ROOT / name.decode("utf-8") for name in output.split(b"\0") if name})
    baseline = json.loads(BASELINE.read_text(encoding="utf-8"))["existing"]
    new = additions(inventory(ROOT, paths), baseline)
    for (path, entry), count in sorted(new.items()):
        print(
            f"{path}: {count} new lint suppression(s): {entry}; remove the suppression "
            "and fix the code; existing exceptions still await human approval",
            file=sys.stderr,
        )
    return int(bool(new))


if __name__ == "__main__":
    sys.exit(main())
