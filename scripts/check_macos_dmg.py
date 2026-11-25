"""Inspect the app in the distribution image on a macOS runner, without launching it."""

import argparse
import json
import plistlib
import subprocess
import tempfile
from pathlib import Path

TAURI = Path(__file__).resolve().parents[1] / "app/src-tauri"


def declared_minimum(load_commands):
    command = None
    for line in load_commands.splitlines():
        fields = line.split()
        if len(fields) != 2:
            continue
        key, value = fields
        if key == "cmd":
            command = value
        elif (command, key) in {
            ("LC_BUILD_VERSION", "minos"),
            ("LC_VERSION_MIN_MACOSX", "version"),
        }:
            return value
    raise ValueError("Mach-O has no macOS minimum deployment version")


def version(value):
    numbers = tuple(map(int, value.split(".")))
    return numbers + (0,) * (3 - len(numbers))


def inspect_bundle(bundle, arch, whisper):
    with (bundle / "Contents/Info.plist").open("rb") as file:
        info = plistlib.load(file)
    identifier = json.loads((TAURI / "tauri.conf.json").read_text())["identifier"]
    if info["CFBundleIdentifier"] != identifier or not info.get("NSMicrophoneUsageDescription", "").strip():
        raise ValueError(f"{bundle.name}: expected {identifier} and a microphone usage description")
    executable = bundle / "Contents/MacOS" / info["CFBundleExecutable"]
    actual_arch = subprocess.check_output(["lipo", "-archs", executable], text=True).split()
    if actual_arch != [arch]:
        raise ValueError(f"{executable.name}: expected a {arch} binary, found {actual_arch}")
    if whisper:
        overlay = json.loads((TAURI / "tauri.whisper-macos.conf.json").read_text())
        expected = overlay["bundle"]["macOS"]["minimumSystemVersion"]
        commands = subprocess.check_output(["otool", "-arch", arch, "-l", executable], text=True)
        binary = declared_minimum(commands)
        plist = info["LSMinimumSystemVersion"]
        if not version(plist) == version(binary) == version(expected):
            raise ValueError(f"{arch}: expected macOS {expected}, Info.plist declares {plist}, Mach-O declares {binary}")
        print(f"{bundle.name}: {arch}, minimum macOS {binary} (Mach-O and Info.plist)")
    else:
        print(f"{bundle.name}: {arch}, microphone usage description present")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--arch", required=True, choices=("arm64", "x86_64"))
    parser.add_argument("--whisper", action="store_true")
    args = parser.parse_args()
    images = sorted(args.directory.glob("*.dmg"))
    if not images:
        parser.error(f"no DMG found in {args.directory}")
    scratch = Path(".tmp")
    scratch.mkdir(exist_ok=True)
    for image in images:
        subprocess.run(["hdiutil", "verify", image], check=True)
        with tempfile.TemporaryDirectory(prefix="seamone-dmg-", dir=scratch) as directory:
            mount = Path(directory).resolve()
            subprocess.run(["hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", mount, image], check=True)
            try:
                bundles = list(mount.glob("*.app"))
                if len(bundles) != 1:
                    raise ValueError(f"{image.name}: expected the Seamone app, found {bundles}")
                inspect_bundle(bundles[0], args.arch, args.whisper)
            finally:
                subprocess.run(["hdiutil", "detach", mount], check=True)


if __name__ == "__main__":
    main()
