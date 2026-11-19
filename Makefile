# seamone 맨 위 Makefile, lint, test, build 는 CI 도 같은 명령을 쓴다.
# 처음 받았으면 `make install` 로 Rust 도구(tauri-cli, trunk)를 받는다.

# build.yml 의 TAURI_CLI_VERSION, TRUNK_VERSION 과 같은 판을 쓴다.
TAURI_CLI_VERSION ?= 2.12.6
TRUNK_VERSION ?= 0.22.2

# `make build TAURI_BUILD_ARGS="--bundles dmg"` 처럼 cargo tauri build 에 인자를 더한다.
CARGO_FEATURES ?=
TEST_PATTERN ?=
CARGO_FEATURE_ARGS = $(if $(CARGO_FEATURES),--features $(CARGO_FEATURES))
TAURI_BUILD_ARGS ?=

# 앱 껍데기 crate 는 컴파일할 때 화면 산출물 폴더가 있어야 한다, lint 와 test 는 화면을 빌드하지 않으므로 빈 폴더만 만든다.
UI_DIST = app/ui/dist

.PHONY: install fmt lint test build dev clean

install:
	rustup toolchain install
	cargo install tauri-cli --version $(TAURI_CLI_VERSION) --locked
	cargo install trunk --version $(TRUNK_VERSION) --locked

fmt:
	cargo fmt --all

lint:
	mkdir -p $(UI_DIST)
	cargo fmt --all --check
	node --check app/ui/tests/review.mjs
	cargo clippy --workspace --all-targets $(CARGO_FEATURE_ARGS) -- -D warnings
	cargo clippy -p seamone-core -p seamone-ui --target wasm32-unknown-unknown -- -D warnings

test:
	mkdir -p $(UI_DIST)
	cargo test --workspace $(CARGO_FEATURE_ARGS) $(TEST_PATTERN)

build:
	cargo tauri build $(CARGO_FEATURE_ARGS) $(TAURI_BUILD_ARGS)

dev:
	cargo tauri dev $(CARGO_FEATURE_ARGS)

clean:
	cargo clean
	rm -rf $(UI_DIST)

.PHONY: ui-test
ui-test:
	cd app/ui && trunk build
	mkdir -p .tmp/browser
	TMPDIR="$(CURDIR)/.tmp/browser" node app/ui/tests/review.mjs

# Optional offline desktop inference, not an installer build.
.PHONY: desktop-proof
desktop-proof:
	mkdir -p $(UI_DIST)
	cargo test -p seamone --features desktop-whisper real_desktop_path -- --ignored --nocapture
