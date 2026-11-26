"""Check direct dependency declarations from Cargo, including renamed and gated edges."""

import json
import subprocess
import sys
from pathlib import Path

# The host composes the UI and engine through IPC. Only domain types cross that boundary.
WORKSPACE_IMPORTS = {
    "seamone-core": set(),
    "seamone-ui": {"seamone-core"},
    "seamone-asr": {"seamone-core"},
    "seamone-runtime": {"seamone-core", "seamone-asr"},
    "seamone": {"seamone-core", "seamone-runtime"},
}

# Libraries that implement these adapters belong to their respective crates.
ADAPTER_OWNERS = {
    "cpal": "seamone-runtime",
    "rusqlite": "seamone-runtime",
    "symphonia": "seamone-runtime",
    "hound": "seamone-runtime",
    "rtrb": "seamone-runtime",
    "block2": "seamone-runtime",
    "objc2": "seamone-runtime",
    "objc2-av-foundation": "seamone-runtime",
    "tauri": "seamone",
    "whisper-rs": "seamone-asr",
    "leptos": "seamone-ui",
    "web-sys": "seamone-ui",
    "wasm-bindgen": "seamone-ui",
}

HOST_NORMAL = {"seamone-core", "seamone-runtime", "serde", "serde_json", "tauri"}


def violations(packages):
    members = {package["name"]: Path(package["manifest_path"]).parent for package in packages}
    for package in packages:
        name = package["name"]
        manifest = package["manifest_path"]
        if name not in WORKSPACE_IMPORTS:
            yield f"{manifest}: classify workspace crate {name} in WORKSPACE_IMPORTS"
            continue
        for feature, enabled in package.get("features", {}).items():
            if any(item == "test-support" or item.endswith("/test-support") for item in enabled):
                yield f"{manifest}: {feature}: test-support belongs only to the host's dev-dependency"
        for dep in package["dependencies"]:
            target = dep["name"]  # Cargo reports the package name separately from its import alias.
            kind = dep["kind"] or "normal"
            context = f"{manifest}: {name} -> {target} ({kind}, {dep['target'] or 'all targets'})"
            if "test-support" in dep.get("features", []) and not (
                name == "seamone" and target == "seamone-runtime" and kind == "dev"
            ):
                yield f"{context}: test-support belongs only to the host's dev-dependency"
            if name == "seamone" and kind == "normal" and not (
                target in HOST_NORMAL or target.startswith("tauri-plugin-")
            ):
                yield f"{context}: put native work in seamone-runtime and call seamone_runtime::LibraryService"
            if target in members or dep.get("path"):
                if target not in WORKSPACE_IMPORTS[name]:
                    yield f"{context}: use seamone_runtime::LibraryService in the host, or the shared domain in other crates"
                elif not dep.get("path") or Path(dep["path"]) != members.get(target):
                    yield f"{context}: depend on the workspace copy of {target}"
                continue
            if name == "seamone-core":
                if (target, kind) not in {("serde", "normal"), ("serde_json", "dev")}:
                    yield f"{context}: keep the domain free of adapters and build-time I/O"
                continue
            owner = ADAPTER_OWNERS.get(target)
            if name == "seamone-ui" and target == "hound":
                # The WASM player decodes the WAV returned by the runtime; it does not own files.
                continue
            if target.startswith("objc2-"):
                owner = "seamone-runtime"
            if target.startswith("tauri-"):
                owner = "seamone"
            if owner and owner != name:
                yield f"{context}: import {target} in {owner}; use seamone_runtime::LibraryService for native work and IPC from the UI"


def main():
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version=1"],
        check=True, stdout=subprocess.PIPE,
    )
    metadata = json.loads(result.stdout)
    members = set(metadata["workspace_members"])
    errors = list(violations([p for p in metadata["packages"] if p["id"] in members]))
    for error in errors:
        print(error, file=sys.stderr)
    return int(bool(errors))


if __name__ == "__main__":
    sys.exit(main())
