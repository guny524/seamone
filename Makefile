# seamone 맨 위 Makefile, lint, test, build 는 CI 도 같은 명령을 쓴다.
# 처음 받았으면 `make install` 로 Rust 도구(tauri-cli, trunk)를 받는다.

# 개발 설치와 CI 바이너리 설치가 같은 판을 사용한다.
TAURI_CLI_VERSION ?= 2.12.6
TRUNK_VERSION ?= 0.22.2
export TAURI_CLI_VERSION TRUNK_VERSION

# `make build TAURI_BUILD_ARGS="--bundles dmg"` 처럼 cargo tauri build 에 인자를 더한다.
CARGO_FEATURES ?=
PYTHON ?= $(if $(filter Windows_NT,$(OS)),python,python3)
TEST_PATTERN ?=
CARGO_FEATURE_ARGS = $(if $(CARGO_FEATURES),--features $(CARGO_FEATURES))
# workspace 전체에서 feature를 직접 켜면 host -> runtime 전달 누락이 가려진다.
WORKSPACE_FEATURE_ARGS = $(if $(CARGO_FEATURES),--features $(patsubst desktop-whisper,seamone/desktop-whisper,$(CARGO_FEATURES)))
TAURI_BUILD_ARGS ?=

# 앱 껍데기 crate 는 컴파일할 때 화면 산출물 폴더가 있어야 한다, lint 와 test 는 화면을 빌드하지 않으므로 빈 폴더만 만든다.
UI_DIST = app/ui/dist

.PHONY: check-comments check-lint-policy lint-native-code lint-ios-code lint-code lint-runtime-minimal lint-workspace-code lint-wasm-code lint-format

.PHONY: install install-ci-tools fmt lint lint-native lint-ios check-imports check-runtime-api check-unsafe check-review-writes check-research-deletions check-architecture check-domain check-lint-exceptions test build dev clean

install:
	rustup toolchain install
	cargo install tauri-cli --version $(TAURI_CLI_VERSION) --locked
	cargo install trunk --version $(TRUNK_VERSION) --locked

install-ci-tools:
	bash scripts/install-tools.sh

fmt:
	cargo fmt --all

check-imports:
	$(PYTHON) scripts/check_crate_boundaries.py

check-comments:
	$(PYTHON) scripts/check_comments.py

check-lint-exceptions:
	$(PYTHON) scripts/check_lint_exceptions.py

check-runtime-api:
	$(PYTHON) scripts/check_runtime_api.py
	$(PYTHON) scripts/check_playback_boundary.py

check-unsafe:
	$(PYTHON) scripts/check_unsafe_files.py

check-review-writes:
	$(PYTHON) scripts/check_review_writes.py

check-research-deletions:
	$(PYTHON) scripts/check_research_deletions.py
	$(PYTHON) scripts/test_research_deletions.py

check-architecture:
	$(MAKE) -k check-lint-policy check-research-deletions test-architecture

.PHONY: test-architecture
test-architecture:
	$(PYTHON) scripts/test_crate_boundaries.py
	$(PYTHON) scripts/test_lint_exceptions.py
	$(PYTHON) scripts/test_runtime_api.py
	$(PYTHON) scripts/test_playback_boundary.py
	$(PYTHON) scripts/test_unsafe_files.py
	$(PYTHON) scripts/test_snapshot_baselines.py
	$(PYTHON) scripts/test_review_writes.py
	$(PYTHON) scripts/test_guard_integrity.py
	$(PYTHON) scripts/test_lint_execution.py
	$(PYTHON) scripts/test_comments.py

check-domain:
	cargo check --locked -p seamone-core --lib --target wasm32v1-none

check-lint-policy: check-comments check-imports check-lint-exceptions check-runtime-api check-unsafe check-review-writes

lint-native:
	$(MAKE) -k check-lint-policy lint-native-code

lint-native-code:
	mkdir -p $(UI_DIST)
	cargo clippy --locked -p seamone -p seamone-runtime --all-targets $(WORKSPACE_FEATURE_ARGS) -- -D warnings

# iOS target and SDK are prepared on the CI macOS runner, never installed by this target.
lint-ios:
	$(MAKE) -k check-lint-policy lint-ios-code

lint-ios-code:
	mkdir -p $(UI_DIST)
	cargo clippy --locked -p seamone -p seamone-runtime --target aarch64-apple-ios-sim -- -D warnings

lint:
	$(MAKE) -k check-lint-policy lint-code

lint-runtime-minimal:
	cargo clippy --locked -p seamone-runtime --lib --no-default-features -- -D warnings

lint-code:
	$(MAKE) -k lint-runtime-minimal lint-format lint-workspace-code lint-wasm-code

lint-format:
	cargo fmt --all --check
	node --check app/ui/tests/review.mjs

lint-workspace-code:
	mkdir -p $(UI_DIST)
	cargo clippy --workspace --all-targets $(WORKSPACE_FEATURE_ARGS) -- -D warnings

lint-wasm-code:
	cargo clippy -p seamone-core -p seamone-ui --target wasm32-unknown-unknown -- -D warnings

test:
	mkdir -p $(UI_DIST)
	cargo test --workspace $(WORKSPACE_FEATURE_ARGS) $(TEST_PATTERN)

.PHONY: test-runtime-doc
test-runtime-doc:
	cargo test --locked -p seamone-runtime --doc

.PHONY: check-macos-dmg
check-macos-dmg:
	$(PYTHON) scripts/check_macos_dmg.py "$(DMG_DIR)" --arch "$(MACOS_ARCH)" $(if $(filter desktop-whisper,$(CARGO_FEATURES)),--whisper)

build: check-imports
	cargo tauri build $(CARGO_FEATURE_ARGS) $(TAURI_BUILD_ARGS)

dev:
	cargo tauri dev $(CARGO_FEATURE_ARGS)

clean:
	cargo clean
	rm -rf $(UI_DIST)

.PHONY: ui-test site-preview
ui-test:
	cd app/ui && trunk build
	mkdir -p .tmp/browser
	TMPDIR="$(CURDIR)/.tmp/browser" node app/ui/tests/review.mjs

SITE_PORT ?= 4173
site-preview:
	python3 -m http.server $(SITE_PORT) --bind 127.0.0.1 --directory site

# Optional offline desktop inference, not an installer build.
.PHONY: desktop-proof
desktop-proof:
	mkdir -p $(UI_DIST)
	cargo test -p seamone-runtime --features desktop-whisper real_desktop_path -- --ignored --nocapture
