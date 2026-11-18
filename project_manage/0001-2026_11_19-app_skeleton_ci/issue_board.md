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
- [ ] 이 노트북(macOS arm64)에서 `make lint`, `make test`, `make build` 가 통과하고 `make build` 가 `.app` 과 `.dmg` 를 만든다
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
- TODO
### 4-2. 바뀌어야 할 곳
- TODO

## 5. 방안과 판정
### 5-9. 최종 방안
- TODO(구현 방향은 2-3 에 정해져 있다, 방향 안에서 갈린 선택만 적는다: Leptos 와 Tauri 의 판, `gen/android` 와 `gen/apple` 을 커밋할지, iOS 빌드 명령)

## 6. 사용자 승인
- 해당 없음: 방향은 PD4, PD5 로 정해졌고 새 사람 몫의 결정이 없다

## 7. 할 일과 검증
(순서대로 한다, 7-1 부터 7-9 까지 끝내고 첫 보고를 쓴다, 메인이 커밋하고 push 한 뒤 7-10 을 이어 시킨다)
(판 번호는 기억으로 적지 않고 `cargo search`, `cargo info`, 공식 문서로 지금 판을 확인해 고정한다)
- [ ] 7-1. 도구 설치: `cargo install tauri-cli trunk --locked`, `rustup target add wasm32-unknown-unknown`, 그 밖의 설치(brew, Xcode, JDK, npm 전역)는 하지 않는다 / 검증: `cargo tauri --version`, `trunk --version`, `rustup target list --installed`
- [ ] 7-2. 폴더와 workspace: skill `project-structure` 를 읽고 맨 위 `Cargo.toml`(workspace, 공통 lint 설정), `crates/seamone-core`(도메인 crate, 지금은 앱 이름과 판을 돌려주는 함수 하나와 그 단위 시험), `app/src-tauri`(Tauri 2 껍데기), `app/ui`(Leptos, client side rendering, trunk 로 빌드)를 만든다, 아직 쓰지 않는 crate 는 만들지 않는다 / 검증: `cargo metadata` 가 세 패키지를 보인다
- [ ] 7-3. 화면에서 core 까지 한 줄기: `app/src-tauri` 에 Tauri command 하나(`seamone-core` 의 함수를 부른다)를 두고 `app/ui` 의 Leptos 화면이 그 command 를 불러 앱 이름과 판을 보여 준다, 화면과 껍데기가 주고받는 타입은 `seamone-core` 에 한 번만 정의해 양쪽이 같이 쓴다 / 검증: `cargo tauri dev` 로 창이 뜨고 값이 보인다(창을 띄운 뒤 스스로 내린다), 값이 맞는지는 command 함수의 단위 시험으로 확인
- [ ] 7-4. make target: skill `set-lint` 를 읽고 맨 위 `Makefile` 에 `lint`(`cargo fmt --check`, 모든 crate 와 wasm 타깃의 `cargo clippy -- -D warnings`), `test`(`cargo test --workspace`), `build`(지금 OS 의 `cargo tauri build`), `dev` 를 만든다 / 검증: `make lint`, `make test`, `make build` 통과, `make build` 뒤 `.app` 과 `.dmg` 경로와 크기를 적는다
- [ ] 7-5. CI 의 lint 와 test: `.github/workflows/ci.yml`, push 와 pull_request 에서 ubuntu 로 `make lint`, `make test`(Tauri 의 Linux 의존 패키지 설치 포함, cargo 캐시 사용) / 검증: 7-10
- [ ] 7-6. CI 의 데스크톱 설치 파일: `.github/workflows/build.yml`, pull_request, main push, workflow_dispatch 에서 macOS(arm64 와 x86_64), Windows, Linux 를 matrix 로 `cargo tauri build` 하고 dmg, exe(NSIS), msi, deb, AppImage 를 artifact 로 올린다, 서명은 하지 않는다 / 검증: 7-10
- [ ] 7-7. CI 의 Android: 같은 workflow 의 job, ubuntu 에 JDK 와 Android SDK, NDK 를 action 으로 준비하고 Rust Android 타깃을 더해 `cargo tauri android init` 뒤 apk 를 만들어 artifact 로 올린다(debug 서명이면 기기에 설치된다) / 검증: 7-10
- [ ] 7-8. CI 의 iOS: 같은 workflow 의 job, macOS runner 에서 `cargo tauri ios init` 뒤 서명 없이 시뮬레이터용으로 빌드하고 `.app` 을 artifact 로 올린다 / 검증: 7-10
- [ ] 7-9. `README.md` 3절(빌드)에 make target, CI 가 만드는 설치 파일과 받는 법, 플랫폼마다 지금 되는 것과 안 되는 것(서명 없음)을 적는다, 2-2 구성요소의 "계획" 표시 가운데 이 이슈로 생긴 것을 "있음" 으로 고친다 / 검증: 적은 명령을 그대로 돌려 본다
- [ ] 7-10. CI 확인과 고치기: 메인이 push 했다고 알리면 `gh run list --branch <브랜치>` 와 `gh run view <id> --log-failed` 로 실패한 job 의 로그를 직접 읽고 고친다, 고친 뒤 보고를 쓰면 메인이 다시 push 한다, 같은 job 이 세 번 고쳐도 실패하면 원인 가설과 로그 줄을 4-2 에 적고 다른 job 부터 통과시킨다 / 검증: 3-4 의 CI 기준, `gh run download` 로 받은 artifact 의 이름과 크기를 적는다(받은 파일은 확인 뒤 지운다)
- [ ] 7-11. 8절 수정 요약을 쓰고 마지막 보고를 쓴다

## 8. 수정 요약
- TODO

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
