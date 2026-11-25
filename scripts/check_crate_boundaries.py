"""Check direct dependency declarations from Cargo, including renamed and gated edges."""

import json
import subprocess
import sys

# The host composes the UI and engine through IPC. Only domain types cross that boundary.
WORKSPACE_IMPORTS = {
    "seamone-core": set(),
    "seamone-ui": {"seamone-core"},
    "seamone-asr": {"seamone-core"},
    "seamone": {"seamone-core", "seamone-asr"},
}

# Libraries that implement these adapters belong to their respective crates.
ADAPTER_OWNERS = {
    "cpal": "seamone",
    "rusqlite": "seamone",
    "symphonia": "seamone",
    "tauri": "seamone",
    "whisper-rs": "seamone-asr",
    "leptos": "seamone-ui",
    "web-sys": "seamone-ui",
    "wasm-bindgen": "seamone-ui",
}


def violations(packages):
    members = {package["name"] for package in packages}
    for package in packages:
        name = package["name"]
        manifest = package["manifest_path"]
        if name not in WORKSPACE_IMPORTS:
            yield f"{manifest}: classify workspace crate {name} in WORKSPACE_IMPORTS"
            continue
        for dep in package["dependencies"]:
            target = dep["name"]  # Cargo reports the package name separately from its import alias.
            kind = dep["kind"] or "normal"
            context = f"{manifest}: {name} -> {target} ({kind}, {dep['target'] or 'all targets'})"
            if target in members or dep.get("path"):
                if target not in WORKSPACE_IMPORTS[name]:
                    yield f"{context}: use the shared domain or move adapter work to its owner"
                continue
            if name == "seamone-core":
                if (target, kind) not in {("serde", "normal"), ("serde_json", "dev")}:
                    yield f"{context}: keep the domain free of adapters and build-time I/O"
                continue
            owner = ADAPTER_OWNERS.get(target)
            if target.startswith("tauri-"):
                owner = "seamone"
            if owner and owner != name:
                yield f"{context}: import {target} in {owner} and use the existing crate or IPC interface"


def main():
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--offline", "--no-deps", "--format-version=1"],
        check=True, capture_output=True, text=True,
    )
    metadata = json.loads(result.stdout)
    members = set(metadata["workspace_members"])
    errors = list(violations([p for p in metadata["packages"] if p["id"] in members]))
    for error in errors:
        print(error, file=sys.stderr)
    return int(bool(errors))


if __name__ == "__main__":
    sys.exit(main())
