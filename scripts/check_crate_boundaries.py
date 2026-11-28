"""Check Cargo dependencies and the UI's host and audio boundaries."""

import json
import re
import subprocess
import sys
from hashlib import sha256
from pathlib import Path
from html.parser import HTMLParser

from rust_tokens import tokens
from check_review_writes import string_value


WORKSPACE_IMPORTS = {
    "seamone-core": set(),
    "seamone-ui": {"seamone-core"},
    "seamone-asr": {"seamone-core"},
    "seamone-runtime": {"seamone-core", "seamone-asr"},
    "seamone": {"seamone-core", "seamone-runtime"},
}


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


WEB_OUTPUT_FEATURES = {'HtmlAudioElement', 'HtmlMediaElement', 'HtmlVideoElement',
                       'AudioContext', 'BaseAudioContext', 'AudioBufferSourceNode',
                       'OscillatorNode', 'AudioWorkletNode'}


MEDIA_TAGS = {'audio', 'video', 'iframe', 'embed', 'object'}
UI_IPC_BRIDGE = Path('src/api/tauri.rs')
UI_IPC_BRIDGE_SHA256 = 'f484204e2d81966ce4401b410eff59e275b7645ecd0b883a53724d8d2e1b6744'
TAURI_IPC_GLOBAL = re.compile(r'\b(?:__TAURI__|__TAURI_INTERNALS__)\b')
UI_IPC_ERROR = 'direct Tauri access is forbidden; use crate::api::call_typed and seamone_core::ipc::command'


def media_literal(value):
    lowered = value.lower()
    return lowered in {'audio', 'video'} or 'data:audio' in lowered or 'data:video' in lowered


def ui_html_errors(source):
    class Page(HTMLParser):
        def __init__(self):
            super().__init__()
            self.errors = []
            self.script = False

        def handle_starttag(self, tag, attrs):
            self.script = tag == 'script'
            if tag in MEDIA_TAGS:
                self.errors.append('UI media elements are forbidden; use runtime output')
            if any(media_literal(value) for _, value in attrs if value is not None):
                self.errors.append('UI media URL is forbidden; use runtime output')
            if any(TAURI_IPC_GLOBAL.search(value) for _, value in attrs if value is not None):
                self.errors.append(UI_IPC_ERROR)

        def handle_endtag(self, tag):
            if tag == 'script':
                self.script = False

        def handle_data(self, data):
            if self.script and re.search(r'\b(?:Audio|AudioContext|BaseAudioContext|AudioWorkletNode)\b|data:(?:audio|video)', data, re.I):
                self.errors.append('UI media JavaScript is forbidden; use runtime output')
            if self.script and TAURI_IPC_GLOBAL.search(data):
                self.errors.append(UI_IPC_ERROR)

    page = Page()
    page.feed(source)
    return page.errors


def ui_output_errors(source):
    code = list(tokens(source))
    for index, token in enumerate(code):
        if token.text in WEB_OUTPUT_FEATURES | {'Audio'}:
            yield 'UI media API is forbidden; use runtime play_recording/control_playback/get_playback_status'
        if token.text in MEDIA_TAGS and index and code[index - 1].text in {'<', ':'}:
            yield 'UI media elements are forbidden; use runtime output'
        literal = string_value(token)
        if literal is not None and (media_literal(literal) or ui_html_errors(literal) or re.search(r'\b(?:Audio|AudioContext|BaseAudioContext|AudioWorkletNode)\b', literal)):
            yield 'UI media markup or JavaScript is forbidden; use runtime output'


def ui_ipc_errors(path, source):
    if path == UI_IPC_BRIDGE:
        if sha256(source.encode('utf-8')).hexdigest() != UI_IPC_BRIDGE_SHA256:
            yield 'the private Tauri bridge changed; preserve call_typed and review the binding with the boundary check'
        return
    for token in tokens(source):
        value = string_value(token)
        if TAURI_IPC_GLOBAL.search(token.text if value is None else value):
            line = source.count('\n', 0, token.start) + 1
            yield f'{line}: {UI_IPC_ERROR}'


def violations(packages):
    members = {package["name"]: Path(package["manifest_path"]).parent for package in packages}
    for package in packages:
        name = package["name"]
        manifest = package["manifest_path"]
        if name not in WORKSPACE_IMPORTS:
            yield f"{manifest}: classify workspace crate {name} in WORKSPACE_IMPORTS"
            continue
        if name == "seamone-ui":
            aliases = {dep.get("rename") or dep["name"] for dep in package["dependencies"] if dep["name"] == "web-sys"}
            for feature, enabled in package.get("features", {}).items():
                for item in enabled:
                    owner, _, forwarded = item.partition('/')
                    if owner.rstrip('?') in aliases and forwarded in WEB_OUTPUT_FEATURES:
                        yield f"{manifest}: {feature}: UI web-sys output feature {forwarded} is forbidden; use runtime output"
            root = Path(manifest).parent
            html = root / 'index.html'
            if html.is_file():
                for error in ui_html_errors(html.read_text(encoding='utf-8')):
                    yield f"{html}: {error}"
            for path in sorted((root / 'src').rglob('*.rs')):
                source = path.read_text(encoding='utf-8')
                for error in ui_output_errors(source):
                    yield f"{path}: {error}"
                for error in ui_ipc_errors(path.relative_to(root), source):
                    yield f"{path}: {error}"
        for feature, enabled in package.get("features", {}).items():
            if any(item == "test-support" or item.endswith("/test-support") for item in enabled):
                yield f"{manifest}: {feature}: test-support belongs only to the host's dev-dependency"
        for dep in package["dependencies"]:
            target = dep["name"]  # Cargo reports the package name separately from its import alias.
            kind = dep["kind"] or "normal"
            if name == "seamone-ui" and target == "web-sys":
                for feature in sorted(set(dep.get("features", [])) & WEB_OUTPUT_FEATURES):
                    yield f"{manifest}: UI web-sys output feature {feature} is forbidden; use runtime output"
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
