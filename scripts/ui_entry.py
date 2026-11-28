"""Keep the checked UI and the repository's Trunk and Tauri build inputs aligned."""

import json
import os
import tomllib
from pathlib import Path


UI_INDEX = 'index.html'
UI_DIST = 'dist'
TRUNK_CONFIG = {
    'build': {'target': UI_INDEX, 'dist': UI_DIST},
    'serve': {'port': 1420, 'open': False},
}
TAURI_HOST = Path('app/src-tauri')


def read_config(path, parse):
    if path.is_symlink():
        raise ValueError('configuration must be a regular file in the repository')
    value = parse(path.read_text(encoding='utf-8'))
    if not isinstance(value, dict):
        raise ValueError('configuration must be an object')
    return value


def trunk_config_errors(root):
    path = root / 'Trunk.toml'
    try:
        actual = read_config(path, tomllib.loads)
    except (OSError, ValueError) as error:
        yield f'{path}: cannot read the approved Trunk configuration: {error}'
        return
    if actual != TRUNK_CONFIG:
        yield f'{path}: use the approved Trunk configuration with index.html, dist, and no extra build inputs'


def tauri_config_errors(workspace, host, ui):
    if host != workspace / TAURI_HOST or host.resolve() != workspace.resolve() / TAURI_HOST:
        yield f'{host}: the checked Tauri host must be app/src-tauri'
        return
    path = host / 'tauri.conf.json'
    expected_dir = Path(os.path.relpath(ui, host)).as_posix()
    expected_build = {
        'beforeDevCommand': {'script': 'trunk serve', 'cwd': expected_dir},
        'devUrl': 'http://localhost:1420',
        'beforeBuildCommand': {'script': 'trunk build --release', 'cwd': expected_dir},
        'frontendDist': f'{expected_dir}/{UI_DIST}',
    }
    try:
        actual = read_config(path, json.loads)
    except (OSError, ValueError) as error:
        yield f'{path}: cannot read the approved Tauri configuration: {error}'
    else:
        if actual.get('build') != expected_build:
            yield f'{path}: Tauri build and dev must use Trunk and the checked UI directory'

    for overlay in sorted(host.iterdir()):
        name = overlay.name.casefold()
        if not name.startswith('tauri.'):
            continue
        if name == 'tauri.conf.json':
            if overlay.name != name:
                yield f'{overlay}: use the exact spelling tauri.conf.json'
            continue
        if name.endswith(('.conf.json5', '.toml')):
            yield f'{overlay}: use the approved JSON Tauri configuration format'
            continue
        if not name.endswith('.conf.json'):
            continue
        try:
            config = read_config(overlay, json.loads)
        except (OSError, ValueError) as error:
            yield f'{overlay}: cannot read the Tauri overlay: {error}'
            continue
        if 'build' in config:
            yield f'{overlay}: Tauri overlays cannot change the checked frontend build configuration'
