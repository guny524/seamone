# seamone
- 기기 안에서 음성을 글로 받아쓰고, 음성과 글을 기기 밖으로 보내지 않은 채 연합학습(FL, federated learning)으로 받아쓰기 모델을 키우는 앱이다
- 이름은 sea anemone(말미잘)에서 왔다
- 상태: 뼈대를 세우는 단계다, 아래 구조는 계획이고 구성요소마다 "있음" 과 "계획" 을 적는다

## 1. 무엇을 하려는가
- 녹음을 기기 안에서 받아쓴다, 한국어를 가장 잘하고 여러 언어를 지원한다
- 전사 결과를 읽거나 내보내는 데 검수는 필요 없다. 검수를 시작하면 구간을 듣고 맞음 또는 틀림을 고른다. 틀린 구간은 나중에 따로 고칠 수 있다
- macOS, Windows, Linux, Android, iOS 에서 돈다

## 2. 구조
### 2-1. 사용자 data flow
```text
+---------------------- 기기(데스크톱, 폰) ----------------------+
| 녹음 -> 받아쓰기(Transcriber) -> 전사 조각 -> 수정 화면         |
|                                     |                        |
|                  맞음·틀림 검수 -> 틀린 구간 교정             |
|                                     |                        |
|                 사람이 확인한 구간만 기기에서 학습(Trainer)   |
+-------------------------------------|------------------------+
                                      v  가중치 업데이트만 전송
                             +------------------+
                             | FL 서버(집계)     | -> 새 공통 모델을 기기로
                             +------------------+
```
- 음성과 전사문은 기기에 보관한다. 사람이 말한 그대로라고 확인한 전사만 학습에 쓴다. 연합학습을 연결하면 기기에서 모델 변경분에 개인정보 보호용 노이즈를 더하고 서버는 보안 집계 결과를 읽는다
- 폰과 데스크톱은 크기가 다른 모델을 쓰고, 같은 크기끼리 FL 로 합친다
### 2-2. 구성요소
- 앱 껍데기와 화면(있음): `app/src-tauri` 는 Tauri 2, `app/ui` 는 Leptos(Rust 를 WASM 으로 컴파일), 지금은 앱 이름과 판 번호를 보여 주는 첫 화면뿐이다
- 도메인(`crates/seamone-core`, 일부 있음): 화면과 앱 껍데기가 같이 쓰는 타입과 규칙, 지금은 앱 이름과 판 번호 타입뿐이고 녹음 세션, 전사 조각, 수정본은 계획이다
- 받아쓰기 엔진(계획): `Transcriber` interface 뒤에서 기기마다 다른 엔진과 가속기를 쓴다
- 학습 엔진(계획): `Trainer` interface, 얼린 몸통 위의 작은 부분만 기기에서 학습한다
- FL 서버(계획): 업데이트를 모아 합치고, 공개 라벨 데이터로 검증해 나빠진 업데이트를 버린다
- 실험 코드(있음): `research/`의 PyTorch 코드, 시작 모델을 평가하고 사람 확인 전사로 학습하는 경로를 시험한다

## 3. 빌드
- 설치 파일(dmg, exe, msi, deb, AppImage, apk, iOS)은 GitHub Actions 에서 만든다, 노트북에는 Xcode, JDK, Android SDK 가 필요 없다
### 3-1. 처음 받았을 때
- `rustup` 을 먼저 설치한다
- 레포 맨 위에서 `make install` 을 실행한다, `rust-toolchain.toml` 이 정한 Rust, tauri-cli, trunk 를 받는다
### 3-2. make target
- `make dev`: 앱을 개발 모드로 띄운다, 화면은 trunk 가 `localhost:1420` 에서 서버로 띄운다
- `make lint`: 서식(`cargo fmt --check`)과 clippy 를 데스크톱 타깃과 WASM 타깃에서 돌린다, 경고는 오류로 본다
- `make test`: 모든 crate 의 시험을 돌린다
- `make build`: CI 가 데스크톱(macOS, Windows, Linux) 설치 파일을 만드는 명령이다, CI 는 플랫폼마다 `TAURI_BUILD_ARGS` 로 `--target` 과 `--bundles` 를 넘겨 이 명령을 부른다, Android 와 iOS 는 workflow 가 `cargo tauri android build` 와 `cargo tauri ios build` 를 직접 부른다(3-3절), 이 프로젝트는 노트북에서 설치 파일을 만들지 않는다(설치 파일은 GitHub Actions 에서만 만든다는 `project_manage/project_decision.md` 의 결정)
- `make fmt`: 서식을 맞춘다
### 3-3. GitHub Actions
- `ci.yml`: push 와 pull request 마다 `make lint` 와 `make test` 를 Ubuntu 에서 돌린다
- `build.yml`: pull request, main push, 수동 실행(Actions 탭의 build 에서 Run workflow)마다 아래를 서명 없이 만들어 artifact 로 올린다(7일 보관)
  - macOS(arm64, x86_64): dmg
  - Windows: exe(NSIS)와 msi
  - Linux: deb 와 AppImage
  - Android: apk(debug 서명, arm64)
  - iOS: 시뮬레이터용 `.app`(zip)
- 받는 법: Actions 탭에서 실행을 열어 아래 Artifacts 에서 받거나 `gh run download <실행 번호>` 를 쓴다
### 3-4. 서명하지 않은 설치 파일
- 이 설치 파일은 Apple 과 Windows 인증서로 서명하지 않아 운영체제가 실행 전에 경고한다
- macOS: 내려받은 `.app` 에서 `xattr -dr com.apple.quarantine seamone.app` 를 실행한 뒤 연다
- Windows: SmartScreen 경고에서 추가 정보, 실행을 차례로 누른다
- Linux: AppImage 는 `chmod +x` 로 실행 권한을 준 뒤 실행한다
- Android: debug 키로 서명한 apk 라서 기기의 설정에서 출처를 알 수 없는 앱 설치를 허용한 뒤 설치한다, 서명 키는 레포의 `.github/android/debug.keystore`(시험용으로 공개된 키, release 에 쓰지 않는다)로 모든 실행에서 같아 앞 실행의 apk 위에 덮어 설치할 수 있다
- iOS: 시뮬레이터용만 만든다, 실기기 설치와 앱 장터 배포는 Apple 개발자 계정과 서명이 필요하다

## 4. 기록
- 진행 상태와 결정은 `project_manage/` 에 있다: `project_board.md`(지금 그림과 남은 일), `project_decision.md`(결정), `project_log.md`(과정), 이슈마다 폴더 하나

## 5. 라이선스
- Apache-2.0, `LICENSE`
