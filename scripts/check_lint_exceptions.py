"""Freeze suppressions and lint configuration against a readable base commit."""

import argparse
import collections
import io
import json
import os
import re
import subprocess
import sys
import tokenize
import tomllib
from pathlib import Path

from guard_integrity import base_commit
from rust_tokens import attributes

ROOT = Path(__file__).resolve().parent.parent
BASELINE = "scripts/lint_exceptions.json"
ALTERNATE = {"clippy.toml", ".clippy.toml", "ruff.toml", ".ruff.toml"}


def rust_exceptions(source):
    for attribute in attributes(source):
        yield re.sub(r"\s+", " ", attribute).strip()


def python_exceptions(source):
    for token in tokenize.generate_tokens(io.StringIO(source).readline):
        if token.type == tokenize.COMMENT and re.search(
            r"\bnoqa\b|\btype\s*:\s*ignore\b", token.string, re.IGNORECASE
        ):
            yield token.string.strip()


def configuration(path, source):
    data = tomllib.loads(source)
    if path.name == "Cargo.toml":
        return {"lints": data.get("lints", {}),
                "workspace.lints": data.get("workspace", {}).get("lints", {})}
    return data.get("tool", {}).get("ruff", {}).get("lint", {})


def configuration_exceptions(path, source):
    data = configuration(path, source)
    if path.name == "Cargo.toml":
        for group in data.values():
            for tool, rules in group.items():
                if tool == "workspace":
                    continue
                for rule, value in rules.items():
                    level = value.get("level") if isinstance(value, dict) else value
                    if level == "allow":
                        yield f"{tool}.{rule}=allow"
    else:
        for key in ("ignore", "extend-ignore", "per-file-ignores", "extend-per-file-ignores"):
            value = data.get(key)
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


def git(root, *args):
    return subprocess.check_output(["git", *args], cwd=root, stderr=subprocess.PIPE).decode("utf-8")


def clippy_commands(source):
    # Fold continued recipe lines before checking flags.
    source = re.sub(r"\\\n[ \t]*", " ", source)
    return sorted(line.strip() for line in source.splitlines()
                  if line.startswith("\t") and re.search(r"\bcargo\s+clippy\b", line))


def command_errors(path, commands):
    for line in commands:
        if not re.search(r"(?:^|\s)-D\s*warnings(?:\s|$)", line) or re.search(
            r"(?:^|\s)(?:-A|--allow)(?:\S*|\s)", line
        ):
            yield f"{path}: clippy must keep -D warnings and must not add -A/--allow: {line}"


def config_snapshots(root, names, read):
    return {name: configuration(Path(name), read(name)) for name in names
            if Path(name).name in ("Cargo.toml", "pyproject.toml")}


def contains_allow(value):
    if isinstance(value, dict):
        return any(contains_allow(item) for item in value.values())
    if isinstance(value, list):
        return any(contains_allow(item) for item in value)
    return isinstance(value, str) and bool(re.search(r"(?:^|[\s\x1f])(?:-A|--allow)", value))


def relocated(root, commit, entries):
    """Carry an existing allowance only when Git identifies a file move, never a copy."""
    fields = git(root, "diff", "--find-renames=50%", "--name-status", "-z", commit).split("\0")
    renames = {}
    index = 0
    while index < len(fields) and fields[index]:
        status = fields[index]
        count = 3 if status.startswith(("R", "C")) else 2
        if status.startswith("R"):
            renames[fields[index + 1]] = fields[index + 2]
        index += count
    return [[renames.get(path, path), entry] for path, entry in entries]


def configuration_changes(current, previous, *, new_workspace_members=False):
    inherited = {"lints": {"workspace": True}, "workspace.lints": {}}
    return sorted(key for key in set(current or {}) | set(previous or {})
                  if (current or {}).get(key) != (previous or {}).get(key)
                  and not (new_workspace_members and key not in (previous or {})
                           and Path(key).name == "Cargo.toml" and current[key] == inherited))


def native_split_command(command):
    """Expand the pre-runtime recipe to the required runtime scope, never shrink HEAD."""
    command = command.replace("$(CARGO_FEATURE_ARGS)", "$(WORKSPACE_FEATURE_ARGS)")
    if " -p seamone " in command and " -p seamone-runtime " not in command:
        command = command.replace(" -p seamone ", " -p seamone -p seamone-runtime ")
    return command


def check(root, ref):
    try:
        commit = git(root, "rev-parse", "--verify", "--end-of-options", ref + "^{commit}").strip()
        base = json.loads(git(root, "show", f"{commit}:{BASELINE}"))
        base_names = git(root, "ls-tree", "-r", "--name-only", commit).splitlines()
        read_base = lambda name: git(root, "show", f"{commit}:{name}")
        base_config = base.get("configuration")
        if base_config is None:
            # Upgrade the snapshot format using the base's actual configuration, never HEAD's.
            base_config = config_snapshots(root, base_names, read_base)
        base_commands = base.get("clippy_commands")
        if base_commands is None:
            base_commands = clippy_commands(read_base("Makefile"))
    except (subprocess.CalledProcessError, ValueError, KeyError) as error:
        return [f"{BASELINE}: cannot read lint baseline at {ref}: {error}; fetch the base ref and provide a reviewed baseline"]
    names = sorted(set(filter(None, git(root, "ls-files", "--cached", "--others", "--exclude-standard", "-z").split("\0"))))
    paths = [root / name for name in names if (root / name).is_file()]
    baseline = json.loads((root / BASELINE).read_text(encoding="utf-8"))
    errors = []
    for label, current, previous in [
        (BASELINE + " vs " + ref, baseline["existing"], relocated(root, commit, base["existing"])),
        ("source vs " + BASELINE, inventory(root, paths), baseline["existing"]),
    ]:
        for (path, entry), count in sorted(additions(current, previous).items()):
            errors.append(f"{label}: {path}: {count} new lint suppression(s): {entry}; remove the suppression and fix the code")
    read_current = lambda name: (root / name).read_text(encoding="utf-8")
    actual_config = config_snapshots(root, [p.relative_to(root).as_posix() for p in paths], read_current)
    for label, current, previous, new_members in [
        (BASELINE + " configuration vs " + ref, baseline.get("configuration"), base_config, True),
        ("Cargo/Ruff lint configuration vs " + BASELINE, actual_config, baseline.get("configuration"), False),
    ]:
        changed = configuration_changes(current, previous, new_workspace_members=new_members)
        if changed:
            errors.append(f"{label}: lint tables changed (including deletions/thresholds): {', '.join(changed)}")
    actual_commands = clippy_commands(read_current("Makefile"))
    required_commands = baseline.get("clippy_commands", [])
    if (sorted(required_commands) != sorted(map(native_split_command, base_commands))
            or collections.Counter(required_commands) - collections.Counter(actual_commands)):
        errors.append(f"Makefile: clippy recipes differ from {BASELINE} or {ref}; preserve the reviewed commands")
    errors.extend(command_errors("Makefile", actual_commands))
    for path in paths:
        if path.name in ALTERNATE:
            errors.append(f"{path.relative_to(root)}: alternate lint configuration is forbidden; use the reviewed Cargo/Ruff tables")
        if path.name in ("config", "config.toml") and path.parent.name == ".cargo":
            data = tomllib.loads(path.read_text(encoding="utf-8"))
            if contains_allow(data):
                errors.append(f"{path.relative_to(root)}: -A/--allow rustflags are forbidden")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-ref", default=os.environ.get("LINT_BASE_REF", "origin/develop"))
    args = parser.parse_args()
    try:
        base_commit(ROOT, args.base_ref)
    except (ValueError, subprocess.CalledProcessError) as error:
        print(str(error), file=sys.stderr)
        return 1
    try:
        errors = check(ROOT, args.base_ref)
    except (OSError, ValueError, KeyError, tokenize.TokenError) as error:
        errors = [f"lint configuration cannot be checked: {error}"]
    for error in errors:
        print(error, file=sys.stderr)
    return int(bool(errors))


if __name__ == "__main__":
    sys.exit(main())
