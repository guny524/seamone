# 앱 뼈대(Tauri 2 + Leptos)를 세우고 다섯 플랫폼 설치 파일을 CI 로 만든다
(이 파일: 기준 브랜치 develop 의 `project_manage/0001-2026_11_19-app_skeleton_ci/issue_board.md`, 같은 폴더에 `issue_decision.md`, `issue_log.md`)
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
- 시작 시점의 레포에는 코드와 CI 가 없었다, 커밋 158c9d3 가 `README.md`, `LICENSE`, `.gitignore`, `project_manage/` 의 기록만 넣었다
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
- 7-15 의 시험이 결함을 잡는지(2026-11-19 07:2x, 로컬 `cargo test -p seamone`): (1) 새 시험 통과, (2) `generate_handler!` 에서 command 를 일부러 빼면 `Command get_app_info not found` 로 실패, (3) 상수 값을 일부러 `get_app_info_wrong` 으로 바꾸면 `Command get_app_info_wrong not found` 로 실패, 둘 다 되돌렸고 되돌린 뒤 통과했다
- 7-16 의 macOS 앱 첫 화면(2026-11-19 06:47 ~ 07:16): CI 의 `desktop (macos-arm64)` 가 올린 dmg 의 `.app` 을 이 노트북에서 실행했다, 처음에는 창이 맨 앞이 아니어서 안쪽이 비어 보였고(캡처 픽셀 (248,250,252), WebKit 이 가려진 창의 DOM 갱신을 그리지 않은 것으로 추측), 맨 앞으로 올리자 `seamone` 과 `seamone 0.1.0` 이 보였다

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
- [x] 7-5. CI 의 lint 와 test: `.github/workflows/ci.yml`, push 와 pull_request 에서 ubuntu 로 `make lint`, `make test`(Tauri 의 Linux 의존 패키지 설치 포함, cargo 캐시 사용) / 검증: 7-10
  - 결과(7-10 의 CI 실행에서 통과, 이 칸의 파일은 이후 7-12 ~ 7-14 로 고쳤다): `.github/workflows/ci.yml` 에 job `lint-test`(ubuntu-22.04, push 와 pull_request)를 만들었다, `rustup toolchain install`(`:26`)로 Rust 를 받고 apt 로 Tauri 의 Linux 의존 패키지를 설치한 뒤(`:29-32`) `Swatinem/rust-cache@v2`(`:34`), `make lint`(`:37`), `make test`(`:40`)를 돌린다
- [x] 7-6. CI 의 데스크톱 설치 파일: `.github/workflows/build.yml`, pull_request, main push, workflow_dispatch 에서 macOS(arm64 와 x86_64), Windows, Linux 를 matrix 로 `cargo tauri build` 하고 dmg, exe(NSIS), msi, deb, AppImage 를 artifact 로 올린다, 서명은 하지 않는다 / 검증: 7-10
  - 결과(7-10 의 CI 실행에서 통과, 이 칸의 파일은 이후 7-12 ~ 7-14 로 고쳤다): `.github/workflows/build.yml:24-89` 의 job `desktop` 이 matrix 넷(macos-arm64, macos-x86_64, windows-x86_64, linux-x86_64)으로 `cargo tauri build --target <triple> --bundles <종류>`(`:77`)를 돌려 `target/<triple>/release/bundle/` 의 dmg, exe, msi, deb, AppImage 를 artifact 로 올린다(`:79`, 없으면 오류, 7일 보관), tauri-cli 와 trunk 는 `scripts/install-tools.sh`(`:69`)로 받는다, 트리거는 pull_request, main push, workflow_dispatch 다
- [x] 7-7. CI 의 Android: 같은 workflow 의 job, ubuntu 에 JDK 와 Android SDK, NDK 를 action 으로 준비하고 Rust Android 타깃을 더해 `cargo tauri android init` 뒤 apk 를 만들어 artifact 로 올린다(debug 서명이면 기기에 설치된다) / 검증: 7-10
  - 결과(7-10 의 CI 실행에서 통과, 이 칸의 파일은 이후 7-12 ~ 7-14 로 고쳤다): `.github/workflows/build.yml:91-133` 의 job `android`(ubuntu-latest)가 JDK 21(`actions/setup-java@v6`, `:104`)과 runner 에 깔린 NDK(`NDK_HOME=$ANDROID_NDK_LATEST_HOME`, `:118`)로 `cargo tauri android init --ci --skip-targets-install`(`:121`) 뒤 `cargo tauri android build --debug --apk --target aarch64`(`:125`)를 돌려 `app/src-tauri/gen/android/app/build/outputs/apk/**/*.apk` 를 artifact 로 올린다(D6)
- [x] 7-8. CI 의 iOS: 같은 workflow 의 job, macOS runner 에서 `cargo tauri ios init` 뒤 서명 없이 시뮬레이터용으로 빌드하고 `.app` 을 artifact 로 올린다 / 검증: 7-10
  - 결과(7-10 의 CI 실행에서 통과, 이 칸의 파일은 이후 7-12 ~ 7-14 로 고쳤다): `.github/workflows/build.yml:134-173` 의 job `ios`(macos-latest)가 `cargo tauri ios init --ci --skip-targets-install`(`:154`) 뒤 `cargo tauri ios build --debug --target aarch64-sim --no-sign`(`:158`)를 돌리고, 만들어진 `.app` 을 `ditto` 로 zip 에 묶어(`:162`) artifact 로 올린다(D7), 이 노트북에는 Xcode 가 없어 CI 에서만 확인된다
- [x] 7-9. `README.md` 3절(빌드)에 make target, CI 가 만드는 설치 파일과 받는 법, 플랫폼마다 지금 되는 것과 안 되는 것(서명 없음)을 적는다, 2-2 구성요소의 "계획" 표시 가운데 이 이슈로 생긴 것을 "있음" 으로 고친다 / 검증: 적은 명령을 그대로 돌려 본다
  - 결과: `README.md:28-35`(2-2 에서 앱 껍데기와 화면은 "있음", 도메인은 "일부 있음"), `README.md:36-62`(3절 빌드: 3-1 처음 받았을 때, 3-2 make target, 3-3 GitHub Actions, 3-4 서명하지 않은 설치 파일)
  - 돌려 본 명령: `make fmt`, `make lint`, `make test`, `make -n install`, `scripts/install-tools.sh`(실행해 `cargo-tauri` 2.12.6 과 `trunk` 0.22.2 를 설치했다)
  - 돌리지 못한 것: `make build` 와 `make dev`(메인 지시 05:58), README 3-2 의 "macOS 에서는 `target/release/bundle/` 아래 `.app` 과 `.dmg` 가 나온다" 는 Tauri 문서가 정한 위치를 적은 것이라 이 레포에서 확인하지 못했다(7-10 의 artifact 경로로 맞춘다), `make install`(소스 컴파일이라 이 노트북에서는 돌리지 않았다, D3)
- [x] 7-10. CI 확인과 고치기: 메인이 push 했다고 알리면 `gh run list --branch <브랜치>` 와 `gh run view <id> --log-failed` 로 실패한 job 의 로그를 직접 읽고 고친다, 고친 뒤 보고를 쓰면 메인이 다시 push 한다, 같은 job 이 세 번 고쳐도 실패하면 원인 가설과 로그 줄을 4-2 에 적고 다른 job 부터 통과시킨다 / 검증: 3-4 의 CI 기준, `gh run download` 로 받은 artifact 의 이름과 크기를 적는다(받은 파일은 확인 뒤 지운다)
  - 실행(커밋 24ff6d2, 2026-11-19 06:09 에 메인이 push 하고 draft PR 을 열었다): `ci` push 44398448917 과 pull_request 44398471687 의 `lint-test`(4분 38초)가 성공했고, `build` pull_request 44398471809 의 `desktop (macos-arm64)`, `desktop (macos-x86_64)`, `desktop (windows-x86_64)`, `desktop (linux-x86_64)`, `android`, `ios` 여섯 job 이 모두 첫 시도에서 성공했다(4분 9초 ~ 6분 44초), 실패한 job 이 없어 고칠 로그 오류 줄도 없다
  - artifact(`gh run download 44398471809` 로 받은 파일의 이름과 바이트): `seamone-macos-arm64/dmg/seamone_0.1.0_aarch64.dmg` 2,227,221, `seamone-macos-x86_64/dmg/seamone_0.1.0_x64.dmg` 2,378,247, `seamone-windows-x86_64/nsis/seamone_0.1.0_x64-setup.exe` 1,477,948 과 `seamone-windows-x86_64/msi/seamone_0.1.0_x64_en-US.msi` 2,162,688, `seamone-linux-x86_64/deb/seamone_0.1.0_amd64.deb` 2,092,248 과 `seamone-linux-x86_64/appimage/seamone_0.1.0_amd64.AppImage` 80,640,504, `seamone-android-apk/universal/debug/app-universal-debug.apk` 52,237,431(artifact 압축본 16,460,514), `seamone-ios-simulator/seamone-ios-simulator.zip` 15,257,495
  - 내용 확인: APK 의 `lib/` 는 `arm64-v8a/libseamone_lib.so` 하나, iOS zip 의 `seamone.app/seamone` 은 Mach-O arm64 에 `platform IOSSIMULATOR`(minos 15.0)이고, macOS dmg 의 `.app` 은 `CFBundleIdentifier io.github.guny524.seamone`, 버전 0.1.0, ad-hoc 서명, 5.0MB 다, 로그의 경고는 실패가 아니다(iOS 의 `No code signing certificates found` 는 `--no-sign` 때문이고 Android 의 `source value 8 is obsolete` 는 Tauri 가 만든 `gen/android` 템플릿의 Java 8 설정이다)
  - 첫 화면 확인: CI 가 만든 macOS arm64 dmg 의 `.app` 을 이 노트북에서 실행해 창을 맨 앞으로 올리고 캡처하니 `seamone` 과 `seamone 0.1.0` 이 보인다(7-16, 캡처는 이슈 폴더 `.tmp/macos-arm64-app-window.png`), macOS x86_64, Windows, Linux, Android 의 설치 파일은 이 노트북에서 실행하지 못했다(사람 시험)
- [ ] 7-11. 8절 수정 요약을 쓰고 마지막 보고를 쓴다
(아래 7-12 ~ 7-15 는 Codex Sol high 의 읽기 전용 검토(06:25 시작 06:30 끝, 대상은 커밋 24ff6d2 하나)가 낸 지적 네 개를 메인이 원문으로 확인해 만든 칸이다, 검토자는 clippy 와 시험을 sandbox 의 쓰기 거부로 돌리지 못했고 서식 검사는 통과했다고 보고했다, 출처는 이슈 폴더 `.tmp/review-cells-1.md`)
- [ ] 7-12. [낮음, 방향 검토] CI 와 Makefile 의 설치 파일 빌드 명령이 둘이고 로컬 설치 파일 빌드를 안내해 프로젝트 결정 PD4(설치 파일은 GitHub Actions 에서만 만든다)와 어긋난다 / 무엇이: `.github/workflows/build.yml:77` 은 `cargo tauri build --target ... --bundles ...` 를 직접 부르고 `Makefile:34-35` 의 `build` target 은 CI 가 쓰지 않는다, README 3-2 는 "`make build`: 지금 OS 의 설치 파일을 만든다" 라고 안내한다 / 고칠 방향: desktop job 의 그 단계가 `make build TAURI_BUILD_ARGS="--target ${{ matrix.target }} --bundles ${{ matrix.bundles }}"` 를 부르게 해 Makefile 을 설치 파일 빌드 명령의 유일한 정의로 두고, README 3-2 의 `make build` 줄을 "CI 가 설치 파일을 만드는 명령이다, 이 프로젝트는 로컬에서 설치 파일을 만들지 않는다(PD4)" 로 바꾼다, Windows runner 에 `make` 가 없어 실패하면 그 단계 앞에 `if: runner.os == 'Windows'` 로 `choco install make -y` 를 더한다 / 검증: CI 의 desktop 네 job 이 통과하고 artifact 네 개가 그대로 올라온다
  - 고친 것(CI 확인 대기): `.github/workflows/build.yml:83-85` 의 단계 `Build the installers` 가 `make build TAURI_BUILD_ARGS="--target ${{ matrix.target }} --bundles ${{ matrix.bundles }}"` 를 `shell: bash` 로 부른다(전: `cargo tauri build --target ... --bundles ...` 를 직접 불렀다), `:65-67` 에 Windows 전용 단계 `choco install make --version=4.4.1 -y --no-progress` 를 더했다(Chocolatey 커뮤니티 feed 의 `make` 최신 판이 4.4.1 이다, Windows runner 이미지 목록에 make 가 없다), `README.md:45` 의 `make build` 줄을 "CI 가 설치 파일을 만드는 명령이고 이 프로젝트는 노트북에서 설치 파일을 만들지 않는다" 로 바꿨다(전: "지금 OS 의 설치 파일을 만든다, macOS 에서는 `target/release/bundle/` 아래 `.app` 과 `.dmg` 가 나온다")
  - 남은 검증: CI 의 desktop 네 job 이 통과하고 artifact 네 개가 그대로 올라오는지(push 뒤, Windows 에서 `make` 가 잡히는지가 불확실하다)
- [ ] 7-13. [중간, 결함 검토] CI 가 고정되지 않은 외부 코드를 실행한다 / 무엇이: (a) `scripts/install-tools.sh:59-67` 의 `install_from` 은 받은 압축 파일의 해시를 확인하지 않고 `install` 한다, (b) `.github/workflows/build.yml` 과 `ci.yml` 의 `uses:` 12곳이 이동 가능한 태그(`actions/checkout@v7`, `Swatinem/rust-cache@v2`, `actions/setup-java@v6`, `actions/upload-artifact@v7`)다 / 영향(추론): 같은 URL 의 파일이나 태그가 바뀌면 레포 변경 없이 다른 코드가 CI 에서 돌아 설치 파일을 바꿀 수 있다 / 고칠 방향: (a) 플랫폼마다 tauri-cli 2.12.6 과 trunk 0.22.2 의 압축 파일 sha256 을 스크립트에 적고 받은 파일이 다르면 설치하지 않고 1 로 끝나게 한다(값은 `curl -fsSL <url> | shasum -a 256` 으로 이번에 구하고 릴리스에 체크섬 파일이 있으면 그것과도 대조한다, 대조 도구는 `sha256sum` 과 `shasum -a 256` 중 있는 것), (b) 모든 `uses:` 를 `gh api repos/<소유자>/<레포>/commits/<태그> --jq .sha` 로 구한 commit SHA 로 고정하고 뒤에 `# v7` 처럼 태그를 주석으로 단다 / 검증: 일부러 틀린 해시로 스크립트를 한 번 돌려 거절하는지 보고(로컬 macOS 분기, 설치 위치는 임시 폴더) 되돌린다, `rg -n 'uses:' .github/workflows` 의 모든 줄이 40자리 SHA 이고 태그 주석이 있다, CI 가 통과한다
  - 고친 것: (a) `scripts/install-tools.sh` 에 `sha256_of` 와 `expected_sha256`(여섯 값: tauri-cli 2.12.6 과 trunk 0.22.2 의 Linux, macOS arm64, Windows 압축 파일)을 더하고, `install_from` 이 받은 파일의 sha256 이 다르거나 그 판의 값이 없으면 설치하지 않고 1 로 끝나게 했다, 값은 `curl -fsSL <url> | shasum -a 256` 으로 구했고 trunk 의 세 값은 릴리스의 `<파일>.sha256` 과 같다, tauri-cli 릴리스에는 체크섬 파일이 없다, (b) `uses:` 13곳(`ci.yml` 2곳, `build.yml` 11곳, 7-14 가 더한 `actions/cache` 포함)을 `gh api repos/<소유자>/<레포>/commits/<태그> --jq .sha` 로 구한 commit SHA 로 고정하고 `# v7.0.1` 꼴의 태그 주석을 달았다(checkout v7.0.1, upload-artifact v7.0.2, setup-java v6.3.0, rust-cache v2.9.3, cache v6.1.0 이고 이동 태그 v7, v6, v2 가 가리키던 SHA 와 같다)
  - 로컬 검증(임시 설치 폴더, 되돌림): 맞는 해시로 돌리면 `cargo-tauri` 와 `trunk` 가 설치된다, 맥 tauri-cli 의 해시를 0 으로 바꾼 사본을 돌리면 `cargo-tauri-aarch64-apple-darwin.zip 의 sha256 이 다르다, 기대 000...0, 받은 것 1c71c48a..., 설치하지 않는다` 로 거절하고 설치된 파일이 0개다, `TAURI_CLI_VERSION=9.9.9` 로 돌리면 그 판의 값이 없다며 종료 코드 1 로 거절한다, `rg -n 'uses:' .github/workflows` 13줄이 모두 `@<40자리 SHA> # v<판>` 꼴이다(아닌 줄 0개), `bash -n` 과 `shellcheck` 지적 0건, 두 workflow 의 YAML 문법 통과
  - 남은 검증: CI 가 통과하는지(push 뒤)
- [ ] 7-14. [중간, 결함 검토] Android CI 의 debug 서명 키가 실행마다 새로 생겨 앞 실행의 APK 위에 덮어 설치하지 못한다 / 무엇이: `.github/workflows/build.yml:91-125` 의 android job 은 매번 새 runner 에서 `cargo tauri android init` 뒤 `--debug` 로 빌드해 Gradle 이 `~/.android/debug.keystore` 를 새로 만든다, 앞 실행의 APK 가 깔린 기기에 다른 실행의 APK 를 덮어 설치하면 인증서가 달라 거부된다(추론, APK 두 개로 재현하지는 못했다) / 고칠 방향: android job 에서 `actions/cache`(SHA 로 고정)로 `~/.android/debug.keystore` 를 고정 key `android-debug-keystore-v1` 로 복원하고 저장한다(첫 실행은 miss 라 Gradle 이 만든 키가 저장되고 다음 실행은 같은 키를 쓴다), 이 키는 시험용이고 release 에 쓰지 않는다는 주석을 단다, 키 파일이 있는지와 sha256 을 로그에 찍는 단계를 빌드 뒤에 더한다 / 검증: 첫 실행에서 cache miss 와 키의 sha256, `gh run rerun <run id>` 로 같은 커밋을 다시 돌린 두 번째 실행에서 cache hit 과 같은 sha256, 결과를 board 4-2 에 적는다
  - 고친 것(CI 확인 대기): `.github/workflows/build.yml:124-129`(android job)에 `actions/cache@55cc8345... # v6.1.0`(id `debug-keystore`)으로 `~/.android/debug.keystore` 를 고정 key `android-debug-keystore-v1` 로 복원하고 저장하는 단계를 빌드 앞에 더했고, `:153-157` 의 단계(artifact 를 올린 뒤에 둬서 키 파일이 없어 실패해도 APK 는 올라간다)가 `cache-hit` 출력과 키 파일의 `ls -l`, `sha256sum` 을 로그에 찍는다, 주석에 "이 키는 시험용이고 release 에 쓰지 않는다" 와 "캐시는 브랜치 범위 안에서만 공유되어 main 에서 처음 저장된 뒤에는 pull request 도 같은 키를 받는다" 를 적었다
  - 남은 검증: 첫 실행의 cache miss 와 키 sha256, 같은 실행을 `gh run rerun <run id>` 로 다시 돌린 두 번째 실행의 cache hit 과 같은 sha256(push 뒤, 결과는 4-2 에 적는다)
- [x] 7-15. [낮음, 결함 검토] command 시험이 실제 IPC 연결의 누락을 잡지 못한다 / 무엇이: `app/src-tauri/src/lib.rs:17-24` 의 시험은 Rust 함수 `get_app_info()` 를 직접 불러서, `generate_handler!` 에 command 를 등록하지 않았거나 화면(`app/ui/src/app.rs:14-18`)의 `"get_app_info"` 문자열이 틀려도 통과한다 / 고칠 방향: `seamone-core` 에 command 이름 상수를 한 번 정의해(`pub const COMMAND_GET_APP_INFO: &str = "get_app_info";`) 화면의 `invoke` 호출이 그 상수를 쓰게 하고, `app/src-tauri` 의 시험에서 `tauri::test`(mock runtime, `tauri` 의 `test` feature 는 dev-dependencies 에만)로 등록된 handler 에 그 상수 이름으로 IPC 요청을 보내 응답이 `AppInfo` 로 해석되고 `seamone_core::app_info()` 와 같은지 본다, 증상만 덮는 길(시험 기대값 느슨하게 하기)은 쓰지 않는다 / 검증: (1) 새 시험이 통과, (2) `generate_handler!` 에서 command 를 일부러 빼면 새 시험이 실패한다(되돌린다), (3) 상수 값을 일부러 바꾸면 실패한다(되돌린다), 세 결과를 board 4-2 에 적는다, `CARGO_BUILD_JOBS=1 nice -n 10 cargo test -p seamone` 으로 돌리고 로컬이 메모리 부족으로 안 되면 CI 로 (1)만 확인하고 (2), (3)은 "남음" 으로 적는다
  - 고친 것: `crates/seamone-core/src/lib.rs:13` 에 `pub const COMMAND_GET_APP_INFO: &str = "get_app_info";` 를 더하고, `app/ui/src/app.rs:15` 의 `invoke` 가 그 상수를 쓰게 했다(전: 문자열 `"get_app_info"` 를 직접 적었다), `app/src-tauri/src/lib.rs` 는 `generate_handler!` 등록을 `register_commands` 함수로 뽑아 `run()` 과 시험이 같은 등록을 쓰게 하고(`:9-12`), 시험을 Rust 함수 직접 호출에서 `tauri::test`(mock runtime)로 등록된 handler 에 `COMMAND_GET_APP_INFO` 이름으로 IPC 요청을 보내 응답을 `AppInfo` 로 읽어 `seamone_core::app_info()` 와 비교하는 것으로 바꿨다(`:23-58`), `app/src-tauri/Cargo.toml` 의 `[dev-dependencies]` 에 `tauri` 의 `test` feature 를 더했다
  - 시험을 쓰며 겪은 것: 문서 예제처럼 요청 주소를 `http://tauri.localhost` 로 고정하면 macOS 에서 `get_app_info not allowed. Plugin not found` 로 거절됐다(그 주소가 이 플랫폼의 로컬 주소가 아니면 Tauri 의 ACL 검사가 원격 요청으로 보고 command 를 막는다, `crates/tauri/src/webview/mod.rs` 의 `on_message`), 요청 주소를 창이 열린 주소(`webview.url()`)로 바꿔 플랫폼마다 맞는 로컬 주소가 되게 했다
  - 검증(로컬, `CARGO_BUILD_JOBS=1 nice -n 10 cargo test -p seamone`): (1) 새 시험 `registered_command_answers_over_ipc_with_the_core_app_info` 통과, (2) `generate_handler!` 에서 command 를 일부러 뺀 사본은 `Command get_app_info not found` 로 실패, (3) 상수 값을 일부러 `get_app_info_wrong` 으로 바꾼 사본은 `Command get_app_info_wrong not found` 로 실패, 두 사본 모두 되돌린 뒤 `diff` 로 백업과 같음을 확인했고 같은 시험이 다시 통과했다
  - 남은 검증: CI 의 `make test`(ubuntu-22.04)에서 같은 시험이 통과하는지(push 뒤)
(아래 7-16 은 worker 가 7-10 의 설치 파일 확인 중 직접 찾은 결함이다)
- [x] 7-16. [높음, 실행 확인] CI 가 만든 macOS 앱의 창이 비어 있다(제목 줄 `seamone` 만 보이고 화면 글자가 없다) / 무엇이: CI 의 `desktop (macos-arm64)`(실행 44398471809)가 올린 dmg(`seamone_0.1.0_aarch64.dmg`)의 `.app` 을 이 노트북에서 `open -n` 으로 실행하면 800x600 창이 뜨지만 안쪽이 CSS 배경색(`#f8fafc`, 캡처 픽셀 (248,250,252))만 칠해지고 `h1` 과 `seamone 0.1.0` 이 없다, 같은 코드의 debug 화면 산출물은 WKWebView 에서(`window.__TAURI__.core.invoke` 를 흉내 내) `seamone` 과 `seamone 0.1.0` 을 그린다 / 원인: 아직 모른다, 아래 4-2 에 확인 과정을 적는다 / 고칠 방향: 원인을 찾은 뒤 적는다 / 검증: 고친 코드로 만든 `.app` 을 같은 방법(`open -n` 과 창 캡처)으로 열어 `seamone 0.1.0` 이 보인다
  - 결과(결함이 아니었다): 처음 빈 창은 창이 맨 앞이 아니어서 WebKit 이 DOM 갱신을 그리지 않은 것이다, 같은 `.app` 을 `open -n` 으로 띄우고 `open` 으로 한 번 더 열어 맨 앞으로 올린 뒤 캡처하면 큰 글씨 `seamone` 과 `seamone 0.1.0` 이 보인다, `0.1.0` 은 화면이 Tauri command `get_app_info` 를 실제 IPC 로 불러 받은 값이라 창, 화면(WASM), IPC, command, core 한 줄기가 릴리스 앱에서 통한다(캡처 `.tmp/macos-arm64-app-window.png`)
  - 근거: (1) 같은 코드의 debug 와 release 화면 산출물을 WKWebView 로 열면 http 와 `tauri://` 흉내(Tauri 의 응답 헤더 `Content-Type`, `Access-Control-Allow-Origin` 과 같게, `window.__TAURI__` 는 흉내) 모두 `seamone` 과 `seamone 0.1.0` 을 그린다(release wasm 140,942바이트), (2) 내가 띄운 보이는 창(맨 앞으로 올림)의 캡처에는 글씨가 담긴다, (3) 이 세션은 사용자 입력이 3시간 넘게(HID 대기 11,799초) 없었고 `UserIsActive 0` 이라, `open -n` 만으로 띄운 창은 가려진 것으로 취급됐다고 추측한다(화면 잠금인지는 확인하지 못했다)
  - 고칠 것: 없다, 사람 시험에서 창이 비어 보이면 창을 한 번 눌러 맨 앞으로 올린다

### 7-17. GitHub 이슈에서 이어서 할 작업
- 게시본: [GitHub 이슈 #1](https://github.com/guny524/seamone/issues/1)

#### 목표와 범위
Tauri 2와 Leptos 앱의 첫 화면을 띄우고 macOS, Windows, Linux, Android 설치 파일과 iOS 시뮬레이터용 앱을 GitHub Actions에서 만든다. 녹음·받아쓰기는 #5에서 다룬다. 출시용 서명과 앱 장터 배포는 별도 작업이다.

- [x] Rust workspace, Tauri 2 앱, Leptos 첫 화면과 Tauri command 연결을 main에 병합한다. [앱 뼈대 PR #6](https://github.com/guny524/seamone/pull/6)
- [x] GitHub Actions에서 `make lint`와 `make test`를 통과시킨다. [main CI](https://github.com/guny524/seamone/actions/runs/44428897396)
- [x] macOS Intel·Apple Silicon, Windows, Linux, Android debug, iOS 시뮬레이터 빌드를 통과시키고 아티팩트 목록을 확인한다. [main 설치 파일 빌드](https://github.com/guny524/seamone/actions/runs/44428897129)
- [ ] macOS에서 DMG로 설치하고 첫 화면에 앱 이름과 버전이 표시되는지 확인한다
- [ ] Windows에서 EXE 또는 MSI로 설치하고 첫 화면을 확인한다. Linux에서도 배포판에 맞는 DEB 또는 AppImage로 같은 동작을 확인한다
- [ ] Android 기기에 debug APK를 설치해 첫 화면을 확인한다. iOS 시뮬레이터용 앱은 시뮬레이터에서 첫 화면을 확인한다
- [ ] 플랫폼별 OS 버전, 설치 파일·빌드 링크, 실행 결과를 이슈에 남긴다. 실행 환경이 없으면 해당 플랫폼의 체크박스를 열어 둔다

#### 작업 위치와 검증
main의 `Makefile`, `app/src-tauri/`, `app/ui/`, `.github/workflows/{ci,build}.yml`을 사용한다. 앱을 묶는 검증은 GitHub Actions에서 한다. 설치 파일 보관 기간이 지났으면 같은 커밋의 빌드 실행을 다시 확보한다. #5의 실제 녹음·전사 검증은 별도로 기록한다.

### 7-18. 컴파일과 패키지 단계의 의존 경계

- [x] Cargo가 해석한 직접 의존성의 방향을 검사하고 운영체제 표준 라이브러리가 없는 타깃에서 공용 도메인을 컴파일한다. IPC 명령과 처리 함수, 플랫폼별 권한, 검수 증거, 코드별 lint 예외를 함께 검사한다. [컴파일과 CI 검사 PR (#12)](https://github.com/guny524/seamone/pull/12)
- [x] 실제 코드에 위반을 하나씩 넣어 컴파일러와 Clippy가 여섯 가지를 각각 거절하는지 확인한다. [거절 진단](https://github.com/guny524/seamone/actions/runs/45583994420)
- [ ] IPC 요청과 응답의 타입을 명령별로 연결한다. 현재 화면은 명령 이름을 타입으로 고르지만 인자는 JSON으로 만들고 응답 타입도 호출부에서 정한다. 우선 검수 저장에서 잘못된 인자나 응답 타입을 컴파일러가 거절하고, 기존 JSON 요청과 응답을 그대로 읽는지 시험한다
- [x] 기본·Whisper 포함 패키지를 각각 여섯 작업으로 빌드한다. macOS 두 CPU의 실제 DMG에서 앱 식별자, CPU와 마이크 안내를 확인하고, Whisper 패키지의 최소 macOS 11.0을 실행 파일과 Info.plist 양쪽에서 대조한다. [기본 패키지](https://github.com/guny524/seamone/actions/runs/45582899052) · [Whisper 포함 패키지](https://github.com/guny524/seamone/actions/runs/45582909048) 실제 arm64 DMG에 Intel 또는 macOS 12.0을 요구했을 때 두 번 모두 검사가 실패했다. [잘못된 배포 조건 거절 로그](https://github.com/guny524/seamone/actions/runs/45584580590/job/139102813255)
- [ ] 위의 설치·첫 실행 항목은 별도 시험 기기에서 확인한다. 실제 마이크·모델 선택·터치 검수는 [녹음과 첫 전사 이슈 (#5)](https://github.com/guny524/seamone/issues/5)에서 이어 간다

## 8. 수정 요약
(7-9 까지의 파일 목록이다, 7-10 에서 CI 를 고치면 이 목록을 다시 맞춘다)
- `rust-toolchain.toml`(새 파일): Rust 1.99.1, clippy, rustfmt, wasm32 타깃을 고정
- `Cargo.toml`(새 파일): workspace(멤버 셋, resolver 3), 공통 lint(clippy all 과 pedantic), dev 와 release profile(화면 crate 만 opt-level z)
- `Cargo.lock`(새 파일): 의존 crate 판 고정, 앱이라 커밋한다
- `crates/seamone-core/Cargo.toml`, `crates/seamone-core/src/lib.rs`(새 파일): 화면과 껍데기가 같이 쓰는 `AppInfo` 와 `app_info()`, 단위 시험 3개
- `app/src-tauri/Cargo.toml`, `build.rs`, `src/lib.rs`, `src/main.rs`(새 파일): Tauri 2 껍데기, command `get_app_info`, 모바일 진입점 `run()`, 단위 시험 1개, `cargo tauri dev` 가 `tauri` 와 `tauri-build` 를 `features = []` 표 꼴로 다시 썼다(Tauri CLI 의 feature 동기화, 그대로 둔다)
- `app/src-tauri/tauri.conf.json`, `capabilities/default.json`(새 파일): 창, 화면 빌드 hook(`trunk`, cwd `../ui`), 번들 설정, identifier `io.github.guny524.seamone`
- `.github/android/debug.keystore`(메인이 더한 파일, 검토 지적으로 Android debug 서명 키를 레포에 둠, 시험용으로 공개된 키이고 release 에 쓰지 않는다)
- `app/src-tauri/app-icon.png`, `app/src-tauri/icons/`(새 파일, 52개 740KB): 임시 아이콘 원본(말미잘 모양)과 `cargo tauri icon` 이 만든 플랫폼별 아이콘
- `app/src-tauri/.gitignore`(새 파일): `/gen/`(생성된 모바일 프로젝트와 권한 schema)
- `app/ui/Cargo.toml`, `index.html`, `styles.css`, `Trunk.toml`, `src/main.rs`, `src/app.rs`, `.gitignore`(새 파일): Leptos 화면, 앱 이름과 판을 command 로 받아 그린다
- `Makefile`(새 파일): `install`, `fmt`, `lint`, `test`, `build`, `dev`, `clean`
- `.github/workflows/ci.yml`(새 파일): push 와 pull_request 에서 `make lint`, `make test`(ubuntu-22.04)
- `.github/workflows/build.yml`(새 파일): 데스크톱 넷(macOS arm64 와 x86_64, Windows, Linux), Android apk, iOS 시뮬레이터 `.app` 의 설치 파일 job
- `scripts/install-tools.sh`(새 파일): CI 와 노트북이 tauri-cli 와 trunk 를 공식 릴리스의 미리 빌드된 파일로 받는다
- `README.md`(고침): 2-2 구성요소의 앱 껍데기와 화면을 "있음", 도메인을 "일부 있음" 으로, 3절(빌드)을 3-1 ~ 3-4 로 채움
(아래는 커밋 24ff6d2 뒤에 검토 칸 7-12 ~ 7-15 로 더 고친 파일이다, `Cargo.lock` 은 바뀌지 않았다)
- `.github/workflows/build.yml`(고침): desktop job 이 `make build TAURI_BUILD_ARGS=...` 로 설치 파일을 만든다(Windows 는 `choco install make --version=4.4.1` 을 먼저, 7-12), 모든 `uses:` 를 commit SHA 로 고정(7-13), android job 에 debug keystore 캐시(`actions/cache`, key `android-debug-keystore-v1`)와 지문을 로그에 찍는 단계(7-14)
- `.github/workflows/ci.yml`(고침): `uses:` 둘을 commit SHA 로 고정(7-13)
- `scripts/install-tools.sh`(고침): 받은 압축 파일의 sha256(여섯 값)이 다르거나 값이 없으면 설치하지 않고 1 로 끝난다(7-13)
- `README.md`(고침): 3-2 의 `make build` 줄을 "CI 가 설치 파일을 만드는 명령이고 이 프로젝트는 노트북에서 설치 파일을 만들지 않는다" 로(7-12)
- `crates/seamone-core/src/lib.rs`(고침): command 이름 상수 `COMMAND_GET_APP_INFO` 를 더했다(7-15)
- `app/ui/src/app.rs`(고침): `invoke` 가 그 상수를 쓴다(7-15)
- `app/src-tauri/src/lib.rs`(고침): `register_commands` 를 뽑고, 시험을 `tauri::test` 로 등록된 handler 에 IPC 요청을 보내는 것으로 바꿨다(7-15)
- `app/src-tauri/Cargo.toml`(고침): `[dev-dependencies]` 에 `tauri` 의 `test` feature(7-15)

## 9. 지금 상태
- 메인이 직접 확인한 것: 브랜치가 main 에 merge 됐다(merge 커밋 8f7e917, 2026-11-19 08:36), 마지막 CI 는 `ci` 둘과 `build` 의 job 6개가 모두 성공이고 Android job 의 로그에 `committed keystore: 'true'` 와 레포 키와 같은 sha256 이 찍혔다, 설치 파일 여덟 개의 형식을 받아 `file` 로 확인했다(첫 CI 기준), 로컬 `make lint` 와 `make test` 는 두 번 EXIT=0
- worker 주장(아직 확인 전): CI 가 만든 macOS 앱을 노트북에서 실행해 첫 화면(`seamone`, `seamone 0.1.0`)이 뜬다(캡처 `.tmp/macos-arm64-app-window.png`, 처음 창이 비어 보인 것은 창이 맨 앞이 아니어서로 추측)
- 남은 것: 사람 시험(10-4), 이슈 마무리 코멘트와 끝내기 승인(사람 시험 뒤)
- 판단거리: 앱 identifier(D4)와 임시 아이콘은 사용자 확인 대기(추천 그대로), 새 이슈 후보는 프로젝트 board 5-4

## 10. Closing
### 10-1. 검토
- Codex Sol(`gpt-6.1-sol`) high 읽기 전용 두 번: 첫 검토(06:25, 지적 네 개: 낮음 둘, 중간 둘)는 모두 원문을 열어 맞다고 확인해 받아 고쳤다(커밋 4c4d2e4), 두 번째 검토(08:07 시작 08:13 끝)는 이전 지적 네 개를 고쳐짐 셋과 일부 하나(Android 키가 캐시 하나에만 기대는 것)로 판정하고 새 지적 둘(낮음: README 가 모바일 빌드도 `make build` 로 설명, 중간: 캐시만으로는 Android 키가 보존되지 않는 경로)을 냈다, 둘 다 원문으로 확인해 받아 고쳤다(c2c9e91 는 README 와 workflow, 2337513 는 레포에 시험용 키를 두는 것), 검토 반복은 두 번에서 멈추는 규칙(review skill 4절)이라 세 번째 검토 대신 CI 로 확인했다, 해당 없음으로 둔 지적은 없다, 원문은 `.tmp/review/codex-1.md`, `codex-3.md`
### 10-2. 커밋 직전 증거 대조
- 검증한 판본: 이슈 브랜치 2337513, 그때의 기준 브랜치 0ef6953
- 마지막 수정 뒤 시험: 2337513 는 CI 로 확인했다(`ci` push, `ci` pull_request, `build` pull_request 모두 success, `build` 의 Android job 에서 레포 키 사용 확인), 로컬 시험은 4c4d2e4 에서 `make lint`, `make test` EXIT=0(시험 4개)
- 요구마다(3-1): R1 변경에 있음(화면, command, core 한 줄기와 IPC 시험), R2 변경에 있음(`make lint`, `test`, `build`, CI 의 desktop 이 `make build` 를 부름), R3 변경에 있음(`ci.yml`), R4 변경에 있음(dmg 둘, exe 와 msi, deb 와 AppImage 가 artifact 로 올라옴), R5 변경에 있음(debug apk, 서명 키는 레포의 키), R6 변경에 있음(iOS 시뮬레이터 zip), 요구하지 않은 동작은 임시 아이콘과 `scripts/install-tools.sh` 의 해시 확인(검토 지적으로 더함)
- 최종 방안대로 했는지: 같음(구현 방향 PD4, PD5), 방향 안의 선택은 이슈 decision 의 D1 에서 D11
- 수정 요약(8절)이 실제 diff 의 파일과 같은지: 다름, `.github/android/debug.keystore`(메인이 더한 커밋 2337513)가 8절에 없고 나머지는 8절이 파일을 묶어 적은 차이다(아이콘 52개 등)
- 완료 기준마다(3-4): 로컬 `make lint`, `test` 통과, CI 의 lint 와 test 통과, 설치 파일 여덟 개 artifact 통과, 로컬 `make build` 는 하지 않음(PD4 와 메모리 한계로 CI 확인으로 바꿈, 메인의 결정), 사람 시험은 대기
- 결과: 통과(사람 시험 대기)
### 10-3. merge 직전 재확인
- 기준 브랜치는 10-2 의 판본 뒤로 `project_manage/` 의 기록 커밋만 늘었다(코드와 workflow 는 그대로라 증거를 깨지 않는다), merge 충돌은 없었다
### 10-4. 사람 시험
- 설치 파일을 받아 실행하면 첫 화면이 뜨는지: macOS 는 worker 가 CI 앱을 실행해 확인했다고 보고했다(메인 미확인), Windows, Linux, Android 는 대기, 방법은 README 3-3과 3-4
### 10-5. 자원
- 받은 CI 설치 파일 사본을 지웠다. 이 경로를 이어 쓴 받아쓰기 엔진 공통 코드가 `develop`에 들어간 뒤 2026-11-22에 Orca 작업 사본을 정리했다. 이슈 폴더 `.tmp/`의 확인 도구와 캡처는 설치 시험 뒤 이슈를 닫을 때 지운다
### 10-6. 이슈 마무리 코멘트
- 상태: 사람 시험 뒤에 쓴다(진행 요약은 이슈에 올렸다)
- 관련 MR: https://github.com/guny524/seamone/pull/6 (merge 됨)
### 10-7. 끝내기 승인
- 사람 시험이 있는 이슈라 사용자가 됐다고 해야 끝난다, 대기
### 10-8. 커밋과 merge
- 24ff6d2 feat: Tauri 2 앱 뼈대와 다섯 플랫폼 설치 파일 CI 를 추가, 4c4d2e4 fix: 설치 도구 해시 확인, action SHA 고정, Android 키 고정, IPC 시험 추가, c2c9e91 fix: Android 서명 키를 레포에서 가져오게 하고 README 의 빌드 설명을 맞춤, 2337513 fix: Android debug 서명 키를 레포에 두어 모든 실행에서 같게 함, merge 8f7e917(2026-11-19)

## 11. 참고
- Tauri 2 문서 https://v2.tauri.app, 다섯 플랫폼 묶기와 mobile init
- Leptos 문서 https://leptos.dev, Tauri 와 같이 쓰는 법
