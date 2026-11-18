# 앱 뼈대(Tauri 2 + Leptos)를 세우고 다섯 플랫폼 설치 파일을 CI 로 만든다
(이 파일: 기준 브랜치 main 의 `project_manage/0001-2026_11_19-app_skeleton_ci/issue_board.md`, 같은 폴더에 `issue_decision.md`, `issue_log.md`)
- 이슈: https://github.com/guny524/seamone/issues/1, 유형: impl(범위가 정해진 구현)
- 운영 상태(상태, 브랜치, worktree, worker, 세션, 선행과 후속): 프로젝트 `project_manage/project_board.md` 4절(이슈) #1

## 1. 사용자 지시 원문
- 2026-11-19 "나는 이거 맥이든 윈도우든 리눅스든 다 돌아가고 핸드폰에서도 됐으면 좋겠어."
- 2026-11-19 "근데 이제 결과물이 뭐 맥에 DSG, DMSG 뭐 하여튼 그 설치 파일 뭐 그걸로 들어가고, 윈도우는 뭐 EXE 파일 설치 파일로 뭐 돼야 될 테고, 리눅스는 뭐 대 파일 뭐 그런 걸로 돼야겠지. 근데 그 핸드폰은 안드로이드 APK나 뭐 iOS 걸로 돼야 되니까 멀티 플랫폼 될 수 있는 걸로 하는 게 나을 것 같아. 그리고 만약에 그런 프레임워크 쓰는 게 러스트나 고가 없어도 그래도 강타입 언어로 이제 컴파일 되는 애들이었으면 좋겠어. 러스트면 베스트고."
- 2026-11-19 "GitHub 액션에서 CI로만 빌드를 해.", "노트북에 도구 설치는 하지마. 아이 뭐 카고, 타우리, 트렁크, 왓슨타기 이런건 괜찮아.", "엑스코드와 JDX는 안 설치하는 게 맞지."

## 2. 배경
### 2-1. 배경
- seamone 은 다섯 플랫폼(macOS, Windows, Linux, Android, iOS)에서 돌아야 한다, 다섯 플랫폼 묶기는 기능을 다 넣은 뒤에 깨지면 고치기 가장 비싼 부분이라 빈 앱으로 먼저 확인한다
### 2-2. 목표
- 한 코드에서 dmg, exe 와 msi, deb 와 AppImage, apk, iOS 빌드가 GitHub Actions 로 나온다, 뒤 이슈는 이 뼈대에 기능만 얹는다
### 2-3. 구현 방향
(이미 정해진 것, 출처 프로젝트 `project_manage/project_decision.md`)
- PD4: 설치 파일은 GitHub Actions 에서만 만든다, 노트북에는 cargo 로 받는 도구(cargo-tauri, trunk, wasm 타깃)만 설치한다, Xcode 와 JDK 와 brew 패키지는 설치하지 않는다
- PD5: Rust, 껍데기는 Tauri 2, 화면은 Leptos(WASM)
- PD12: 디스크를 아낀다, 빌드 폴더는 이 worktree 안에만 만들고 worktree 를 지울 때 함께 지워진다
### 2-4. 해치지 말아야 할 것, 기존 구현 의도
- 레포에는 `README.md`, `LICENSE`, `.gitignore`, `project_manage/` 만 있다, `project_manage/` 는 메인만 고친다(이 board 의 4, 5절과 7절 체크, 8절, `issue_log.md` 는 worker 가 쓴다)
- `research/` 폴더와 `research/Makefile` 은 #2 가 만든다, 이 이슈는 만들지 않는다

## 3. 목표와 범위
### 3-1. 요구
- R1. 한 Rust workspace 에서 Tauri 2 앱이 뜨고 Leptos 화면이 도메인 crate 의 함수를 Tauri command 로 불러 결과를 보여 준다, 출처 PD5
- R2. `make lint`, `make test`, `make build` 가 있다, 출처 rule `execute.md` 1절
- R3. GitHub Actions 가 push 와 PR 마다 lint 와 test 를 돌린다, 출처 rule `change.md` 6절
- R4. GitHub Actions 가 macOS dmg, Windows exe(NSIS)와 msi, Linux deb 와 AppImage 를 만들어 artifact 로 올린다, 출처 사용자 2026-11-19
- R5. GitHub Actions 가 Android apk 를 만들어 artifact 로 올린다, 출처 사용자 2026-11-19
- R6. GitHub Actions 에서 iOS 빌드가 서명 없이 시뮬레이터용으로 통과한다, 출처 사용자 2026-11-19
### 3-2. 범위
- `Cargo.toml`(workspace), `crates/seamone-core`, `app/src-tauri`, `app/ui`, 맨 위 `Makefile`, `.github/workflows/`, `README.md` 3절(빌드)
### 3-3. 제외 범위
- 녹음, 받아쓰기, 학습 기능, 이유: #3, #5 와 뒤 이슈가 맡는다
- 서명된 설치 파일과 앱 장터 배포, 이유: Apple 과 Windows 인증서가 필요하고 사용자가 정할 일이다
- 폰 실기기 시험, 이유: 이 노트북에 Xcode 와 JDK 가 없다
### 3-4. 완료 기준
- [ ] 이 노트북(macOS arm64)에서 `make lint`, `make test` 가 통과한다(`make build` 는 노트북 메모리 때문에 로컬에서 하지 않고 설치 파일은 CI 로만 확인한다, 메인 지시 2026-11-19 05:58)
- [ ] GitHub Actions 의 lint 와 test 가 통과한다
- [ ] GitHub Actions 가 macOS dmg, Windows exe 와 msi, Linux deb 와 AppImage 를 artifact 로 올린다
- [ ] GitHub Actions 가 Android apk 를 artifact 로 올린다
- [ ] GitHub Actions 에서 iOS 시뮬레이터용 빌드가 통과한다, 통과시키지 못하면 막힌 단계와 로그 줄을 4-2 에 적고 "남음" 으로 둔다
- [ ] 사람 시험: 설치 파일을 받아 실행하면 첫 화면이 뜬다(macOS, Windows, Linux, Android)
### 3-5. 검증 방법
- 로컬: `make lint`, `make test`, `make build`
- CI: 메인이 브랜치를 push 한 뒤 `gh run list --branch <브랜치>`, `gh run view <id> --log-failed`, `gh run download <id>` 로 artifact 이름과 크기를 본다
### 3-6. 의존성과 관련 링크
- 후속: #3 받아쓰기 엔진 interface 와 속도 측정(이 workspace 에 crate 를 더한다), #5 녹음과 받아쓰기와 수정 화면
### 3-7. 확인 필요
- 사실: 이 노트북에는 rustc 1.99.1, cargo, node 22, bun, cmake 가 있고 Xcode(명령줄 도구만 있음), JDK, cargo-tauri, trunk, wasm 타깃이 없다, 근거 2026-11-19 03:50 조회
- 가정: `cargo tauri android init` 과 `cargo tauri ios init` 을 CI 에서 매번 돌려 `gen/` 을 만들어도 빌드가 된다, 틀리면 CI 가 만든 `gen/` 을 artifact 로 받아 레포에 커밋하는 길로 바꾼다
- 가정: iOS 는 서명 없이 시뮬레이터용으로 빌드할 수 있다, 틀리면 Rust 쪽 iOS 타깃 컴파일 확인까지만 하고 남은 것을 적는다
- 질문: 없음
### 3-8. 우선 참조할 파일
- `/Users/min-jo/Documents/Developments/seamone/project_manage/project_decision.md` PD4, PD5, PD12
- skill `project-structure`(폴더 자리), `set-lint`(make target), `design-test`(시험 수준)

---
(아래는 AI 가 쓰는 절)

## 4. 조사
### 4-1. 기존 코드와 구현의 핵심, 의도
(`git ls-files` 와 Tauri 저장소의 tag `tauri-cli-v2.12.6` 소스로 확인한 것)
- 시작 시점의 레포에는 코드와 CI 가 없었다, 커밋 98ccde9 가 `README.md`, `LICENSE`, `.gitignore`, `project_manage/` 의 기록만 넣었다
- `.gitignore`(맨 위): `.tmp/`, `target/`, `dist/` 를 이미 뺀다, 그래서 `app/ui/dist` 와 workspace 의 `target/` 은 따로 빼지 않는다
- Tauri 2 가 정한 구조(출처: Tauri 저장소의 `crates/tauri-cli/src/helpers/app_paths.rs`, `crates/tauri-cli/src/build.rs`, create-tauri-app 저장소의 `templates/_base_/src-tauri` 와 `templates/template-leptos`)
  - Android 와 iOS 는 껍데기 crate 의 `#[cfg_attr(mobile, tauri::mobile_entry_point)] pub fn run()` 을 불러 앱을 띄운다, 그래서 껍데기 crate 는 `crate-type = ["staticlib", "cdylib", "rlib"]` 와 `_lib` 로 끝나는 lib 이름을 가져야 하고 `main.rs` 는 그 lib 의 `run()` 만 부른다
  - `cargo tauri` 는 현재 폴더 아래 깊이 3 안에서 `tauri.conf.json` 을 찾는다(`app_paths.rs` 의 `lookup`), 그래서 레포 맨 위에서 돌려도 `app/src-tauri/tauri.conf.json` 이 걸린다
  - `beforeBuildCommand` 와 `beforeDevCommand` 의 `cwd` 는 `src-tauri` 폴더 기준이다(CLI 가 hook 을 돌리기 전에 작업 폴더를 `src-tauri` 로 바꾼다, `build.rs:166`, `dev.rs:148`), `cwd` 를 안 쓰면 `package.json` 이 없는 이 레포에서는 `app/` 에서 돈다
  - 화면(WASM)은 `window.__TAURI__.core.invoke` 로 껍데기의 command 를 부른다, 이 전역 객체는 `withGlobalTauri: true` 일 때만 창에 들어간다
- 모바일 프로젝트 템플릿(tag `tauri-cli-v2.12.6`): Android 는 Gradle 9.6.1, AGP 9.3.1, compileSdk 37 이라 JDK 17 이상이 필요하다
### 4-2. 바뀌어야 할 곳
(새 파일만 있다, 고친 파일은 `README.md` 둘뿐이다)
- `rust-toolchain.toml`, `Cargo.toml`: Rust 판 고정과 workspace(세 crate, 공통 lint, profile)
- `crates/seamone-core`: 화면과 껍데기가 같이 쓰는 `AppInfo` 타입과 `app_info()`, 그 단위 시험
- `app/src-tauri`: Tauri 2 껍데기, command `get_app_info` 하나, 설정, 권한, 아이콘
- `app/ui`: Leptos 화면, trunk 설정
- `Makefile`: `install`, `fmt`, `lint`, `test`, `build`, `dev`, `clean`
- `.github/workflows/ci.yml`, `.github/workflows/build.yml`, `scripts/install-tools.sh`: CI
- `README.md`: 2-2 구성요소의 "있음" 표시와 3절(빌드)
- 메인 결정(2026-11-19 05:58, 근거 PD4 설치 파일은 GitHub Actions 에서만 만든다): 노트북 메모리가 한계라(swap 14.8GB, 부하 30 이상, 디스크 여유 13GB) 로컬 `make build` 와 `make dev` 의 창 확인은 하지 않는다, 설치 파일과 첫 화면은 CI 와 사람 시험으로 확인하고 7-3 의 검증은 단위 시험과 컴파일 검사(`make lint` 의 clippy)로 대신한다
- 로컬 검증이 막힌 경위(worker 가 겪은 것): `make test` 가 05:14 에 시작해 05:26 전에 Claude Code 가 시스템 메모리 부족으로 SIGTERM 으로 멈췄다(그때 스왑 14.1GiB 를 15GiB 가운데 썼고 빈 메모리 페이지는 62MB 였다), 05:35 에 `CARGO_BUILD_JOBS=2 nice -n 10` 으로 다시 돌려 통과했다, `make dev` 첫 시도는 화면(WASM) 첫 컴파일이 Tauri CLI 의 개발 서버 대기 한도 180초를 넘겨 `Could not connect to http://localhost:1420/ after 180s` 로 실패했다, 둘째 시도는 앱 컴파일(341개 가운데 304개) 중에 메인 지시로 내렸다

## 5. 방안과 판정
(구현 방향은 2-3 에 정해져 있어 방향 안에서 갈린 선택만 나눈다)
### 5-1. Leptos 와 Tauri 의 판
- 장점: 안정판(tauri 2.12.7, tauri-cli 2.12.6, Leptos 0.8.23)은 PD5 의 "Tauri 2 + Leptos" 그대로고 create-tauri-app 의 Leptos 템플릿과 같은 판이다
- 단점: 없음
- 판정: 채택 -> 5-9, 기각: tauri 3.0.0-alpha 와 Leptos 0.9.0-rc1 은 시험판이라 뒤 이슈가 기댈 뼈대로 쓰지 않는다
### 5-2. `gen/android` 와 `gen/apple` 을 커밋할지
- 장점(CI 가 매번 만든다): 뼈대에 손으로 고칠 것이 없고 레포에 Gradle 과 Xcode 생성물이 들어가지 않는다
- 단점: 마이크 권한처럼 생성된 프로젝트를 고쳐야 하면 그때 커밋으로 바꿔야 한다
- 판정: 채택 -> 5-9(board 3-7 의 첫 가정), 커밋하는 쪽은 #5 가 생성된 프로젝트를 고쳐야 할 때 다시 본다
### 5-3. 서명 없는 iOS 시뮬레이터 빌드 명령
- 장점(`cargo tauri ios build --debug --target aarch64-sim --no-sign`): tauri-cli 2.12.6 의 `--help` 에 두 옵션이 있고, Tauri 저장소의 모바일 CI 가 같은 명령을 쓴다
- 단점: 이 노트북에는 Xcode 가 없어 CI 에서만 확인된다
- 판정: 채택 -> 5-9, 이 명령이 CI 에서 실패하면 board 3-7 의 둘째 가정(Rust 쪽 iOS 타깃 컴파일 확인까지만)으로 물러선다
### 5-4. tauri-cli 와 trunk 를 받는 방법
- 장점(공식 릴리스의 미리 빌드된 파일): 몇 초면 받고 CI 의 일곱 job 이 같은 바이너리를 쓴다
- 단점: crates.io 가 아니라 GitHub 릴리스에서 받는다
- 판정: 채택 -> 5-9, 기각: `cargo install` 은 job 마다 10분 넘게 컴파일한다(노트북에서도 20분 가까이 걸려 멈췄다), `cargo binstall` 은 tauri-cli 의 binstall 메타데이터에 aarch64-apple-darwin 항목이 없어(릴리스 파일은 zip 인데 기본값이 tgz) macOS arm64 runner 에서 소스 컴파일로 떨어진다
### 5-5. lint 와 test 가 화면 산출물 폴더에 기대는 방법
- 장점(빈 `app/ui/dist` 만 만든다): CI 의 lint job 에 trunk 설치와 WASM 빌드가 들지 않는다
- 단점: 껍데기가 빈 화면을 품고 컴파일된다
- 판정: 채택 -> 5-9, 기각: lint 와 test 마다 `trunk build` 를 도는 것은 시간만 늘고 두 명령은 화면을 쓰지 않는다
### 5-9. 최종 방안
- 위 선택과 `issue_decision.md` D1~D11(결정자 worker, 사용자 확인은 D4 만 대기)으로 만든다: Rust workspace 에 `seamone-core`, Tauri 2 껍데기 `seamone`, Leptos 화면 `seamone-ui`, 맨 위 `Makefile`, `.github/workflows/ci.yml` 과 `build.yml`
- 이유: 화면과 껍데기가 주고받는 타입을 `seamone-core` 에 한 번만 정의해 뒤 이슈가 기능을 얹을 때 양쪽을 따로 고치지 않게 하고, 다섯 플랫폼 묶기는 기능이 없는 지금 CI 로 먼저 확인해 깨지면 고치는 값이 가장 작을 때 잡는다

## 6. 사용자 승인
- 해당 없음: 방향은 PD4, PD5 로 정해졌고 새 사람 몫의 결정이 없다

## 7. 할 일과 검증
(순서대로 한다, 7-1 부터 7-9 까지 끝내고 첫 보고를 쓴다, 메인이 커밋하고 push 한 뒤 7-10 을 이어 시킨다)
(판 번호는 기억으로 적지 않고 `cargo search`, `cargo info`, 공식 문서로 지금 판을 확인해 고정한다)
- [x] 7-1. 도구 설치: `cargo install tauri-cli trunk --locked`, `rustup target add wasm32-unknown-unknown`, 그 밖의 설치(brew, Xcode, JDK, npm 전역)는 하지 않는다 / 검증: `cargo tauri --version`, `trunk --version`, `rustup target list --installed`
  - 결과: `rust-toolchain.toml:1-5`(Rust 1.99.1, clippy, rustfmt, wasm32 타깃)를 `rustup toolchain install` 로 받았다, tauri-cli 2.12.6 과 trunk 0.22.2 는 공식 릴리스의 미리 빌드된 파일을 `scripts/install-tools.sh` 로 `~/.cargo/bin` 에 설치했다(D3, `cargo install` 은 소스 컴파일이 20분 가까이 걸려 멈췄다), brew, Xcode, JDK, npm 전역은 설치하지 않았다
  - 검증 출력: `cargo tauri --version` -> `tauri-cli 2.12.6`, `trunk --version` -> `trunk 0.22.2`, `rustup target list --installed` -> `aarch64-apple-darwin` 과 `wasm32-unknown-unknown`
- [x] 7-2. 폴더와 workspace: skill `project-structure` 를 읽고 맨 위 `Cargo.toml`(workspace, 공통 lint 설정), `crates/seamone-core`(도메인 crate, 지금은 앱 이름과 판을 돌려주는 함수 하나와 그 단위 시험), `app/src-tauri`(Tauri 2 껍데기), `app/ui`(Leptos, client side rendering, trunk 로 빌드)를 만든다, 아직 쓰지 않는 crate 는 만들지 않는다 / 검증: `cargo metadata` 가 세 패키지를 보인다
  - 결과: `Cargo.toml:2-3`(workspace, resolver 3, 멤버 셋), `crates/seamone-core`(`AppInfo` 와 `app_info()`), `app/src-tauri`(Tauri 2 껍데기, 패키지 이름 `seamone`), `app/ui`(Leptos 화면), 빼는 파일이 사는 폴더마다 `.gitignore`(`app/src-tauri/.gitignore` 의 `/gen/`, `app/ui/.gitignore` 의 `/dist/`), skill `project-structure` 를 읽고 따랐다
  - 검증 출력: `cargo metadata --format-version 1 --no-deps` 가 `seamone-core 0.1.0`, `seamone 0.1.0`, `seamone-ui 0.1.0` 세 패키지를 보였다
- [x] 7-3. 화면에서 core 까지 한 줄기: `app/src-tauri` 에 Tauri command 하나(`seamone-core` 의 함수를 부른다)를 두고 `app/ui` 의 Leptos 화면이 그 command 를 불러 앱 이름과 판을 보여 준다, 화면과 껍데기가 주고받는 타입은 `seamone-core` 에 한 번만 정의해 양쪽이 같이 쓴다 / 검증: `cargo tauri dev` 로 창이 뜨고 값이 보인다(창을 띄운 뒤 스스로 내린다), 값이 맞는지는 command 함수의 단위 시험으로 확인
  - 결과: `crates/seamone-core/src/lib.rs:14-26`(`AppInfo` 타입과 `app_info()`), `app/src-tauri/src/lib.rs:3-7`(command `get_app_info` 가 `seamone_core::app_info()` 를 그대로 돌려준다)와 `:9-16`(`run()` 이 `generate_handler!` 로 command 를 등록), `app/ui/src/app.rs:9-11`(`window.__TAURI__.core.invoke` 바인딩), `:14-19`(`fetch_app_info` 가 command 를 불러 `AppInfo` 로 읽는다), `:22-40`(`App` 이 앱 이름과 판을 그린다)
  - 검증(메인 지시 05:58 에 따라 창 확인 대신): `make test` 가 시험 4개를 통과했다, `seamone-core` 3개(이름이 seamone 이고 판 번호가 주.부.수 숫자 셋인지, JSON 필드가 `name` 과 `version` 둘뿐인지, JSON 왕복 뒤 같은 값인지)와 `seamone` 1개(command 가 core 의 값을 그대로 돌려주는지), 화면 코드의 컴파일과 clippy 는 `make lint` 의 WASM 타깃 검사가 통과했다
  - 못 한 것: `cargo tauri dev` 로 창을 띄워 값이 보이는지 확인하는 일, 첫 시도는 화면(WASM) 첫 컴파일이 Tauri CLI 의 개발 서버 대기 한도 180초를 넘겨 실패했고 둘째 시도는 앱 컴파일 중에 메인 지시로 내렸다, 창에 값이 보이는지는 3-4 의 사람 시험으로 남는다
- [x] 7-4. make target: skill `set-lint` 를 읽고 맨 위 `Makefile` 에 `lint`(`cargo fmt --check`, 모든 crate 와 wasm 타깃의 `cargo clippy -- -D warnings`), `test`(`cargo test --workspace`), `build`(지금 OS 의 `cargo tauri build`), `dev` 를 만든다 / 검증: `make lint`, `make test`, `make build` 통과, `make build` 뒤 `.app` 과 `.dmg` 경로와 크기를 적는다
  - 결과: `Makefile:16-42` 에 `install`, `fmt`, `lint`, `test`, `build`, `dev`, `clean` 을 만들었다(`make -n <target>` 으로 각 명령을 확인), `lint`(`:24-28`)는 `cargo fmt --all --check`, 데스크톱 clippy(`--workspace --all-targets -- -D warnings`), WASM clippy(`-p seamone-core -p seamone-ui --target wasm32-unknown-unknown -- -D warnings`)이고 `test`(`:30-32`)는 `cargo test --workspace` 다, 둘 다 화면 산출물 폴더만 `mkdir -p app/ui/dist` 로 만든다(D11)
  - 검증: `make lint` 통과(05:13 EXIT=0 16분 37초, 06:01 `CARGO_BUILD_JOBS=1 nice -n 10` EXIT=0 29.7초, 경고 0건), 첫 실행은 `app/ui/Cargo.toml` 에 `wasm-bindgen-futures` 가 빠져 컴파일 오류가 나서 `app/ui/Cargo.toml:16` 에 더해 고쳤다, `make test` 통과(05:35 `CARGO_BUILD_JOBS=2 nice -n 10` EXIT=0 5분 17초, 06:02 `CARGO_BUILD_JOBS=1 nice -n 10` EXIT=0 44.2초, 시험 4개 통과, 앞서 05:14 에 시작한 첫 실행은 시스템 메모리 부족으로 Claude Code 가 멈췄다)
  - 하지 않은 것: `make build`(릴리스 빌드)는 메인 지시(05:58)로 로컬에서 하지 않았다, `.app` 과 `.dmg` 의 경로와 크기는 CI 의 macOS job 이 올린 artifact 로 7-10 에서 적는다
- [ ] 7-5. CI 의 lint 와 test: `.github/workflows/ci.yml`, push 와 pull_request 에서 ubuntu 로 `make lint`, `make test`(Tauri 의 Linux 의존 패키지 설치 포함, cargo 캐시 사용) / 검증: 7-10
  - 진행(동작 확인은 7-10): `.github/workflows/ci.yml` 에 job `lint-test`(ubuntu-22.04, push 와 pull_request)를 만들었다, `rustup toolchain install`(`:26`)로 Rust 를 받고 apt 로 Tauri 의 Linux 의존 패키지를 설치한 뒤(`:29-32`) `Swatinem/rust-cache@v2`(`:34`), `make lint`(`:37`), `make test`(`:40`)를 돌린다
- [ ] 7-6. CI 의 데스크톱 설치 파일: `.github/workflows/build.yml`, pull_request, main push, workflow_dispatch 에서 macOS(arm64 와 x86_64), Windows, Linux 를 matrix 로 `cargo tauri build` 하고 dmg, exe(NSIS), msi, deb, AppImage 를 artifact 로 올린다, 서명은 하지 않는다 / 검증: 7-10
  - 진행(동작 확인은 7-10): `.github/workflows/build.yml:24-89` 의 job `desktop` 이 matrix 넷(macos-arm64, macos-x86_64, windows-x86_64, linux-x86_64)으로 `cargo tauri build --target <triple> --bundles <종류>`(`:77`)를 돌려 `target/<triple>/release/bundle/` 의 dmg, exe, msi, deb, AppImage 를 artifact 로 올린다(`:79`, 없으면 오류, 7일 보관), tauri-cli 와 trunk 는 `scripts/install-tools.sh`(`:69`)로 받는다, 트리거는 pull_request, main push, workflow_dispatch 다
- [ ] 7-7. CI 의 Android: 같은 workflow 의 job, ubuntu 에 JDK 와 Android SDK, NDK 를 action 으로 준비하고 Rust Android 타깃을 더해 `cargo tauri android init` 뒤 apk 를 만들어 artifact 로 올린다(debug 서명이면 기기에 설치된다) / 검증: 7-10
  - 진행(동작 확인은 7-10): `.github/workflows/build.yml:91-133` 의 job `android`(ubuntu-latest)가 JDK 21(`actions/setup-java@v6`, `:104`)과 runner 에 깔린 NDK(`NDK_HOME=$ANDROID_NDK_LATEST_HOME`, `:118`)로 `cargo tauri android init --ci --skip-targets-install`(`:121`) 뒤 `cargo tauri android build --debug --apk --target aarch64`(`:125`)를 돌려 `app/src-tauri/gen/android/app/build/outputs/apk/**/*.apk` 를 artifact 로 올린다(D6)
- [ ] 7-8. CI 의 iOS: 같은 workflow 의 job, macOS runner 에서 `cargo tauri ios init` 뒤 서명 없이 시뮬레이터용으로 빌드하고 `.app` 을 artifact 로 올린다 / 검증: 7-10
  - 진행(동작 확인은 7-10): `.github/workflows/build.yml:134-173` 의 job `ios`(macos-latest)가 `cargo tauri ios init --ci --skip-targets-install`(`:154`) 뒤 `cargo tauri ios build --debug --target aarch64-sim --no-sign`(`:158`)를 돌리고, 만들어진 `.app` 을 `ditto` 로 zip 에 묶어(`:162`) artifact 로 올린다(D7), 이 노트북에는 Xcode 가 없어 CI 에서만 확인된다
- [x] 7-9. `README.md` 3절(빌드)에 make target, CI 가 만드는 설치 파일과 받는 법, 플랫폼마다 지금 되는 것과 안 되는 것(서명 없음)을 적는다, 2-2 구성요소의 "계획" 표시 가운데 이 이슈로 생긴 것을 "있음" 으로 고친다 / 검증: 적은 명령을 그대로 돌려 본다
  - 결과: `README.md:28-35`(2-2 에서 앱 껍데기와 화면은 "있음", 도메인은 "일부 있음"), `README.md:36-62`(3절 빌드: 3-1 처음 받았을 때, 3-2 make target, 3-3 GitHub Actions, 3-4 서명하지 않은 설치 파일)
  - 돌려 본 명령: `make fmt`, `make lint`, `make test`, `make -n install`, `scripts/install-tools.sh`(실행해 `cargo-tauri` 2.12.6 과 `trunk` 0.22.2 를 설치했다)
  - 돌리지 못한 것: `make build` 와 `make dev`(메인 지시 05:58), README 3-2 의 "macOS 에서는 `target/release/bundle/` 아래 `.app` 과 `.dmg` 가 나온다" 는 Tauri 문서가 정한 위치를 적은 것이라 이 레포에서 확인하지 못했다(7-10 의 artifact 경로로 맞춘다), `make install`(소스 컴파일이라 이 노트북에서는 돌리지 않았다, D3)
- [ ] 7-10. CI 확인과 고치기: 메인이 push 했다고 알리면 `gh run list --branch <브랜치>` 와 `gh run view <id> --log-failed` 로 실패한 job 의 로그를 직접 읽고 고친다, 고친 뒤 보고를 쓰면 메인이 다시 push 한다, 같은 job 이 세 번 고쳐도 실패하면 원인 가설과 로그 줄을 4-2 에 적고 다른 job 부터 통과시킨다 / 검증: 3-4 의 CI 기준, `gh run download` 로 받은 artifact 의 이름과 크기를 적는다(받은 파일은 확인 뒤 지운다)
- [ ] 7-11. 8절 수정 요약을 쓰고 마지막 보고를 쓴다

## 8. 수정 요약
(7-9 까지의 파일 목록이다, 7-10 에서 CI 를 고치면 이 목록을 다시 맞춘다)
- `rust-toolchain.toml`(새 파일): Rust 1.99.1, clippy, rustfmt, wasm32 타깃을 고정
- `Cargo.toml`(새 파일): workspace(멤버 셋, resolver 3), 공통 lint(clippy all 과 pedantic), dev 와 release profile(화면 crate 만 opt-level z)
- `Cargo.lock`(새 파일): 의존 crate 판 고정, 앱이라 커밋한다
- `crates/seamone-core/Cargo.toml`, `crates/seamone-core/src/lib.rs`(새 파일): 화면과 껍데기가 같이 쓰는 `AppInfo` 와 `app_info()`, 단위 시험 3개
- `app/src-tauri/Cargo.toml`, `build.rs`, `src/lib.rs`, `src/main.rs`(새 파일): Tauri 2 껍데기, command `get_app_info`, 모바일 진입점 `run()`, 단위 시험 1개, `cargo tauri dev` 가 `tauri` 와 `tauri-build` 를 `features = []` 표 꼴로 다시 썼다(Tauri CLI 의 feature 동기화, 그대로 둔다)
- `app/src-tauri/tauri.conf.json`, `capabilities/default.json`(새 파일): 창, 화면 빌드 hook(`trunk`, cwd `../ui`), 번들 설정, identifier `io.github.guny524.seamone`
- `app/src-tauri/app-icon.png`, `app/src-tauri/icons/`(새 파일, 52개 740KB): 임시 아이콘 원본(말미잘 모양)과 `cargo tauri icon` 이 만든 플랫폼별 아이콘
- `app/src-tauri/.gitignore`(새 파일): `/gen/`(생성된 모바일 프로젝트와 권한 schema)
- `app/ui/Cargo.toml`, `index.html`, `styles.css`, `Trunk.toml`, `src/main.rs`, `src/app.rs`, `.gitignore`(새 파일): Leptos 화면, 앱 이름과 판을 command 로 받아 그린다
- `Makefile`(새 파일): `install`, `fmt`, `lint`, `test`, `build`, `dev`, `clean`
- `.github/workflows/ci.yml`(새 파일): push 와 pull_request 에서 `make lint`, `make test`(ubuntu-22.04)
- `.github/workflows/build.yml`(새 파일): 데스크톱 넷(macOS arm64 와 x86_64, Windows, Linux), Android apk, iOS 시뮬레이터 `.app` 의 설치 파일 job
- `scripts/install-tools.sh`(새 파일): CI 와 노트북이 tauri-cli 와 trunk 를 공식 릴리스의 미리 빌드된 파일로 받는다
- `README.md`(고침): 2-2 구성요소의 앱 껍데기와 화면을 "있음", 도메인을 "일부 있음" 으로, 3절(빌드)을 3-1 ~ 3-4 로 채움

## 9. 지금 상태
- 메인이 직접 확인한 것: 없음
- worker 주장(아직 확인 전): 없음
- 남은 것: 7절 전부
- 판단거리: 없음

## 10. Closing
### 10-1. 검토
- TODO
### 10-2. 커밋 직전 증거 대조
- TODO
### 10-3. merge 직전 재확인
- TODO
### 10-4. 사람 시험
- 설치 파일을 받아 실행하면 첫 화면이 뜨는지(macOS, Windows, Linux, Android), 결과 대기
### 10-5. 자원
- TODO
### 10-6. 이슈 마무리 코멘트
- 상태: TODO
### 10-7. 끝내기 승인
- TODO
### 10-8. 커밋과 merge
- TODO

## 11. 참고
- Tauri 2 문서 https://v2.tauri.app, 다섯 플랫폼 묶기와 mobile init
- Leptos 문서 https://leptos.dev, Tauri 와 같이 쓰는 법
