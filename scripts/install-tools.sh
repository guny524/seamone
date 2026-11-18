#!/usr/bin/env bash
# CI 에서 tauri-cli 와 trunk 를 공식 릴리스의 미리 빌드된 파일로 받아 설치한다.
# 판은 환경 변수 TAURI_CLI_VERSION, TRUNK_VERSION 으로 받고(.github/workflows/build.yml 이 정한다),
# 설치 위치는 첫 인자다(기본 ~/.cargo/bin).
# cargo install 은 job 마다 10분 넘게 컴파일해서 받는 쪽을 택했다.
# 받은 압축 파일의 sha256 이 아래 expected_sha256 의 값과 다르면 설치하지 않고 1 로 끝난다.
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

# 받은 파일의 sha256, sha256sum 이 있으면 그것을 쓰고 없으면 shasum 을 쓴다.
sha256_of() {
  if command -v sha256sum > /dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

# 압축 파일 이름과 판을 받아 그 파일의 sha256 을 돌려준다, 모르는 조합이면 아무것도 돌려주지 않는다.
# 판을 올리면 새 판의 값을 `curl -fsSL <url> | shasum -a 256` 으로 구해 바꾼다.
# trunk 는 릴리스에 있는 `<파일 이름>.sha256` 과 같은 값이다, tauri-cli 릴리스에는 체크섬 파일이 없다.
expected_sha256() {
  case "$1@$2" in
    cargo-tauri-x86_64-unknown-linux-gnu.tgz@2.12.6)
      echo "057983293b14d88b29331f96b2e57ce9780f887e7a0b76e255844924f83118dc"
      ;;
    cargo-tauri-aarch64-apple-darwin.zip@2.12.6)
      echo "1c71c48a9aea9616a857c92bab63bc9bce113e348929493ba0bbaa4d3f4ce4c3"
      ;;
    cargo-tauri-x86_64-pc-windows-msvc.zip@2.12.6)
      echo "39636fa68bc00f13dd610fb57688f292e93742978cc35395f2f1be7c540b88a1"
      ;;
    trunk-x86_64-unknown-linux-gnu.tar.gz@0.22.2)
      echo "26fca8a9952e46cd7d8b8b9a2e2c1786c090fc14b8e10b6678ab4d0f9f1a385f"
      ;;
    trunk-aarch64-apple-darwin.tar.gz@0.22.2)
      echo "84238ecc7def5ece23c257a392789ead196121adf4e1e7c25302a3d9c475275a"
      ;;
    trunk-x86_64-pc-windows-msvc.zip@0.22.2)
      echo "d8bb35a396c6bc3df97ce108b945cb4b2f15f71b8be0972efd48b5e02664bcef"
      ;;
  esac
}

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

# 압축 파일(url)을 받아 sha256 을 확인한 뒤 안에 든 실행 파일(name 이나 name.exe)을 bin_dir 에 둔다.
install_from() {
  local url="$1" name="$2" version="$3"
  local archive="$work_dir/${url##*/}"
  local extracted="$work_dir/extracted-$name"
  local expected actual found

  expected="$(expected_sha256 "${archive##*/}" "$version")"
  if [ -z "$expected" ]; then
    echo "${archive##*/} (판 $version)의 sha256 이 이 스크립트에 없다, 판을 올렸으면 expected_sha256 에 새 값을 적는다" >&2
    exit 1
  fi
  curl -fsSL --retry 3 --retry-delay 2 -o "$archive" "$url"
  actual="$(sha256_of "$archive")"
  if [ "$actual" != "$expected" ]; then
    echo "${archive##*/} 의 sha256 이 다르다, 기대 $expected, 받은 것 $actual, 설치하지 않는다" >&2
    exit 1
  fi
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

install_from "https://github.com/tauri-apps/tauri/releases/download/tauri-cli-v${TAURI_CLI_VERSION}/${tauri_asset}" cargo-tauri "$TAURI_CLI_VERSION"
install_from "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/${trunk_asset}" trunk "$TRUNK_VERSION"
