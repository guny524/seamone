#!/usr/bin/env bash
# CI 에서 tauri-cli 와 trunk 를 공식 릴리스의 미리 빌드된 파일로 받아 설치한다.
# 판은 환경 변수 TAURI_CLI_VERSION, TRUNK_VERSION 으로 받고(.github/workflows/build.yml 이 정한다),
# 설치 위치는 첫 인자다(기본 ~/.cargo/bin).
# cargo install 은 job 마다 10분 넘게 컴파일해서 받는 쪽을 택했다.
set -euo pipefail

: "${TAURI_CLI_VERSION:?TAURI_CLI_VERSION 환경 변수가 필요하다}"
: "${TRUNK_VERSION:?TRUNK_VERSION 환경 변수가 필요하다}"

bin_dir="${1:-$HOME/.cargo/bin}"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT
mkdir -p "$bin_dir"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)
    tauri_asset="cargo-tauri-x86_64-unknown-linux-gnu.tgz"
    trunk_asset="trunk-x86_64-unknown-linux-gnu.tar.gz"
    ;;
  Darwin-arm64)
    tauri_asset="cargo-tauri-aarch64-apple-darwin.zip"
    trunk_asset="trunk-aarch64-apple-darwin.tar.gz"
    ;;
  MINGW*-x86_64 | MSYS*-x86_64)
    tauri_asset="cargo-tauri-x86_64-pc-windows-msvc.zip"
    trunk_asset="trunk-x86_64-pc-windows-msvc.zip"
    ;;
  *)
    echo "지원하지 않는 환경이다: $(uname -s)-$(uname -m)" >&2
    exit 1
    ;;
esac

extract() {
  local archive="$1" dest="$2"
  case "$archive" in
    *.tgz | *.tar.gz)
      tar -xzf "$archive" -C "$dest"
      ;;
    *.zip)
      if command -v unzip > /dev/null 2>&1; then
        unzip -q -o "$archive" -d "$dest"
      else
        powershell.exe -NoProfile -Command \
          "Expand-Archive -Force -LiteralPath '$(cygpath -w "$archive")' -DestinationPath '$(cygpath -w "$dest")'"
      fi
      ;;
  esac
}

# 압축 파일(url)을 받아 안에 든 실행 파일(name 이나 name.exe)을 bin_dir 에 둔다.
install_from() {
  local url="$1" name="$2"
  local archive="$work_dir/${url##*/}"
  local extracted="$work_dir/extracted-$name"
  local found

  curl -fsSL --retry 3 --retry-delay 2 -o "$archive" "$url"
  mkdir -p "$extracted"
  extract "$archive" "$extracted"
  found="$(find "$extracted" -type f \( -name "$name" -o -name "$name.exe" \) | head -n 1)"
  if [ -z "$found" ]; then
    echo "$archive 안에 $name 이 없다" >&2
    exit 1
  fi
  install -m 0755 "$found" "$bin_dir/$(basename "$found")"
  echo "설치했다: $bin_dir/$(basename "$found") ($url)"
}

install_from "https://github.com/tauri-apps/tauri/releases/download/tauri-cli-v${TAURI_CLI_VERSION}/${tauri_asset}" cargo-tauri
install_from "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/${trunk_asset}" trunk
