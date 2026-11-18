# 이슈 log
(형식은 `project-issue-conductor` skill 의 `references/record-format.md` 7절, 시각 순으로 쌓고 지우지 않는다)

## 2026-11-19 06:08 worker(app, Sonnet 5.5 max) 보고
- worker app(Sonnet 5.5 max)이 7-1 ~ 7-9 를 했다, 칸마다 바꾼 파일:줄과 방법은 board 7절 각 칸 아래, 파일 목록은 board 8절, 보고 원문은 이슈 폴더 `.tmp/report-1.md`(git 에서 빠지는 임시 파일이라 요점을 아래에 남긴다)
- 끝낸 것: 도구 설치(tauri-cli 2.12.6, trunk 0.22.2, Rust 1.99.1 고정), workspace 셋(`seamone-core`, `seamone`, `seamone-ui`), 화면에서 core 까지 command 한 줄기, `Makefile`, `ci.yml` 과 `build.yml`, `scripts/install-tools.sh`, README 3절
- 검증: `make lint` 통과(05:13 EXIT=0, 06:01 `CARGO_BUILD_JOBS=1 nice -n 10` 으로 다시 EXIT=0), `make test` 통과(05:35 `CARGO_BUILD_JOBS=2 nice -n 10` EXIT=0, 06:02 `CARGO_BUILD_JOBS=1 nice -n 10` EXIT=0, 시험 4개), 두 workflow 의 YAML 과 `scripts/install-tools.sh`(shellcheck)와 JSON 설정의 문법, `cargo tauri ios build --help` 의 `--target aarch64-sim` 과 `--no-sign`, `cargo tauri android build --help` 의 `--target aarch64`, `--apk`, `--debug`
- 겪은 일: `make lint` 첫 실행이 `wasm-bindgen-futures` 누락으로 실패해 `app/ui/Cargo.toml:16` 에 더해 고쳤다, `make test` 첫 실행(05:14 시작)은 시스템 메모리 부족으로 Claude Code 가 멈췄다(스왑 14.1GiB 를 15GiB 가운데 썼다), `make dev` 첫 시도는 화면 WASM 첫 컴파일이 Tauri CLI 의 개발 서버 대기 한도 180초를 넘겨 실패했고 둘째 시도는 앱 컴파일(341개 중 304개) 중에 메인 지시(05:58)로 내렸다
- 메인 지시: 05:30 메모리 부족 뒤 `CARGO_BUILD_JOBS=2 nice -n 10` 으로 다시 돌리라는 지시, 05:58 로컬 무거운 일을 줄이라는 지시(`make build` 와 창 확인은 하지 않고 설치 파일은 CI 로만 확인, 근거 PD4)를 따랐다
- 남은 것: CI 전부(7-5 ~ 7-8, 7-10), 창에 값이 보이는지(사람 시험), `make build` 의 `.app` 과 `.dmg` 경로와 크기(CI artifact 로 확인), `make install`(소스 컴파일이라 이 노트북에서 돌리지 않음), 시험이 결함을 잡는지 일부러 어긴 구현으로 확인하는 일(`design-test` 6절, 메모리 때문에 안 함)
- 판단거리와 추천: build.yml 이 돌려면 PR 이 있어야 한다(추천 draft PR), 앱 identifier `io.github.guny524.seamone` 사용자 확인 대기(추천 그대로), 아이콘은 임시(추천 사용자 디자인이 올 때까지), 화면 CSP `null`(추천 #5 전에 새 이슈), `gen/` 미커밋은 #5 가 생성된 프로젝트를 고칠 때 다시 본다
- 자원: 이 worktree 의 `target/` 3.5GB 는 worktree 와 함께 지워진다(PD12), `~/.rustup/toolchains/1.99.1-aarch64-apple-darwin` 562MB 는 이 레포가 stable 과 같은 판을 따로 고정해 생겼다(지울 조건: 이 레포 작업이 끝났을 때 `rustup toolchain uninstall 1.99.1`)
