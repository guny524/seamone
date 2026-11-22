# 받아쓰기 엔진 interface 를 세우고 기기별 속도를 잰다
(이 파일: 기준 브랜치 develop 의 `project_manage/0003-2026_11_19-asr_engine_speed/issue_board.md`, 같은 폴더에 `issue_decision.md`, `issue_log.md`)
- 이슈: https://github.com/guny524/seamone/issues/3, 유형: impl(범위가 정해진 구현)
- 운영 상태(상태, 브랜치, worktree, worker, 세션, 선행과 후속): 프로젝트 `project_manage/project_board.md` 4절(이슈) #3

## 1. 사용자 지시 원문
- 2026-11-19 "그리고 이제 아까 최종적으로 보고 있는 모델 크기는 CPU에서 돌아가는 모델 크기였으면 좋겠어. 한 3빌리언 정도면 그래도 CPU에서 빨리 돌지 않을까?"
- 2026-11-19 "폰에서 폰 AP 가속기 써가지고 못해? 최대한 일단 할 수 있는 거에서 모델 크기가 컸으면 좋겠어. 아니면은 폰과 노트북 모델은 따로 가는 걸로 하자. 그 노트북이나 그런 데서도 맥 같은 거는 GPU 그 AP 가속기를 쓸 수가 있잖아. M1 이런 것들은 그런 거 좀 감안을 하고 우리 CPU에서 계산할 때도 심드 같은 걸로 최대한 가속해서 계산할 수 있게끔 하고"
- 2026-11-19 "그리고 최대한 고나 로스트 같은 걸로 그냥 빌드가 됐으면 좋겠고,"

## 2. 배경
### 2-1. 배경
- 기기마다 쓸 수 있는 가속기가 다르다(Apple GPU, 폰 GPU 와 NPU, 가속기 없는 CPU), 엔진을 기기마다 갈아 끼우려면 받아쓰기 interface 가 먼저 있어야 한다
- 시작 모델은 "쓸 수 있는 속도 안에서 가장 큰 모델" 로 고른다(프로젝트 PD7), 공개된 속도 수치는 기기와 엔진이 제각각이라 앱에 실제로 들어갈 엔진으로 직접 잰다
### 2-2. 목표
- `Transcriber` interface 뒤에서 엔진 둘 이상이 같은 음성을 받아쓰고, 후보 모델마다 이 노트북의 CPU 와 가속기 속도가 나온다, #2 의 오류율과 합쳐 계층마다 시작 모델을 추천한다
### 2-3. 구현 방향
(이미 정해진 것, 출처 프로젝트 `project_manage/project_decision.md`)
- PD5: Rust, PD7: 가속기를 쓰고 CPU 는 SIMD 로 가속한다, 속도 기준은 데스크톱 계층이 가속기로 실시간의 3배 이상과 CPU 만으로 실시간 이상, 폰 계층이 가속기로 실시간 이상
- interface 의 계약: 입력은 16kHz mono 음성 표본과 언어 힌트(없을 수 있다), 출력은 전사 조각의 목록(시작 시각, 끝 시각, 글, 화자 칸은 비워 둔다), 조각의 타입은 `seamone-core` 에 한 번만 정의한다
- 엔진은 cargo feature 로 켜고 끈다, 기본 feature 만으로 다섯 플랫폼의 빌드와 CI 가 그대로 통과해야 한다(C++ 를 빌드하는 엔진은 기본으로 켜지 않는다)
- 엔진 후보: whisper.cpp 를 감싼 crate(CPU SIMD 와 Apple GPU), 순수 Rust 엔진(Candle 의 Whisper, `qwen-asr` crate), 지금 판과 유지 상태는 조회해 확인한다
### 2-4. 해치지 말아야 할 것, 기존 구현 의도
- #1 이 만든 workspace, Makefile, CI 를 깨지 않는다
- 모델 파일은 레포에 넣지 않는다, `/Users/min-jo/Documents/Developments/seamone/.tmp/` 아래에 받고 측정이 끝나면 지운다(PD12)

## 3. 목표와 범위
### 3-1. 요구
- R1. `seamone-asr` crate 에 `Transcriber` interface 와 엔진 구현 둘 이상이 있다, 출처 2-3
- R2. 속도 측정 CLI 가 엔진, 모델, 장치(CPU 만, 가속기), 스레드 수를 받아 음성 길이, 처리 시간, 실시간 배수(음성 길이 나누기 처리 시간), 메모리 최고치를 낸다, 출처 PD7
- R3. 후보 모델(Whisper base, small, large-v3-turbo, large-v3, Qwen3-ASR 0.6B 와 1.7B 가운데 엔진이 받는 것)을 같은 한국어 음성 10개로 재고, 첫 실행은 빼고 세 번 잰 가운데값을 쓴다, 출처 PD7
- R4. interface 와 실시간 배수 계산에 단위 시험이 있다(엔진 없이 가짜 엔진으로), 출처 rule `change.md` 6절
- R5. #2 의 오류율과 합쳐 계층마다 시작 모델을 추천한다, 폰 계층은 실측이 없으므로 어림임을 밝힌다, 출처 PD9
### 3-2. 범위
- `crates/seamone-asr`, `crates/seamone-core` 의 조각 타입, 속도 측정 CLI, 결과 문서(자리는 skill `project-structure` 로 정한다), `README.md` 2-2 의 받아쓰기 엔진 줄
### 3-3. 제외 범위
- 화면과 녹음, 이유: #5
- 학습, 이유: 뒤 이슈
- 폰 실측, 이유: 기기와 빌드 환경이 없다
### 3-4. 완료 기준
- [ ] `make lint`, `make test` 통과, 기본 feature 로 CI 의 다섯 플랫폼 빌드가 그대로 통과
- [ ] 엔진 둘이 같은 한국어 음성 하나를 받아써 글이 나온다
- [ ] 후보 모델마다 CPU 만 쓸 때와 Apple GPU 를 쓸 때의 실시간 배수와 메모리가 결과 문서에 있다, 재지 못한 조합은 까닭이 있다
- [ ] 계층마다 시작 모델 추천이 PD7 의 기준과 #2 의 오류율에 근거해 적혀 있다
### 3-5. 검증 방법
- `make lint`, `make test`, 측정은 make target 하나(`make bench-asr` 같은 이름)로 다시 돌릴 수 있게 한다
### 3-6. 의존성과 관련 링크
- 선행: #1 앱 뼈대, #2 오류율 측정(결과와 한국어 음성 표본), 후속: #5 녹음과 받아쓰기와 수정 화면, 프로젝트 PD9
### 3-7. 확인 필요
- 사실: 이 노트북에 cmake 가 있다, Xcode 는 명령줄 도구만 있다
- 가정: 명령줄 도구만으로 Apple GPU(Metal)를 쓰는 엔진이 빌드된다, 틀리면 CPU 측정만 하고 가속기 측정은 "재지 못함" 과 까닭을 적는다
- 질문: 없음
### 3-8. 우선 참조할 파일
- `/Users/min-jo/Documents/Developments/seamone/project_manage/project_decision.md` PD5, PD7, PD9, PD12
- `research/results/2026_11_19_start_model_cer.md`(#2 가 merge 된 뒤)
- skill `contract-spec`(interface 와 DTO), `project-structure`, `design-test`

---
(아래는 AI 가 쓰는 절)

## 4. 조사
### 4-1. 기존 코드와 구현의 핵심, 의도
(`git ls-files` 와 `git grep` 으로 확인한 것, 이 worktree 는 앱 뼈대(#1)의 커밋 4c4d2e4 위다)
- 받아쓰기 코드는 레포에 없었다(`git grep -n -i transcri HEAD -- crates app Makefile scripts .github` 0건), `README.md:31` 에 "받아쓰기 엔진(계획)" 한 줄뿐이다
- `crates/seamone-core/src/lib.rs`: 화면과 앱 껍데기가 같이 쓰는 타입을 `serde` 만으로 정의한다(`AppInfo`), WASM 으로도 컴파일되고 `#![forbid(unsafe_code)]` 다, 의도 앞 이슈의 "화면과 껍데기가 주고받는 타입은 `seamone-core` 에 한 번만 정의", 그래서 전사 조각 타입도 `serde` 만 쓴다
- 맨 위 `Cargo.toml`: workspace 멤버와 공통 lint(clippy all 과 pedantic, `make lint` 가 `-D warnings` 로 돈다), 새 crate 는 `[lints] workspace = true` 로 같은 lint 를 받는다
- `Makefile` 의 `lint` 와 `test` 는 workspace 전체를 돈다(`cargo clippy --workspace --all-targets`, `cargo test --workspace`), 새 crate 는 Makefile 을 고치지 않고 workspace 멤버로만 들어간다
### 4-2. 바뀌어야 할 곳
(이번에 한 것과 남은 것)
- 새 파일(이번에 만들었다): `crates/seamone-core/src/transcript.rs`(전사 조각), `crates/seamone-asr/Cargo.toml`, `crates/seamone-asr/src/lib.rs`(interface, 오류, 입력 검사), `crates/seamone-asr/src/fake.rs`(가짜 엔진), `crates/seamone-asr/src/speed.rs`(순수 함수)
- 고친 파일(추적 중인 것 셋): 맨 위 `Cargo.toml`(workspace 멤버에 `crates/seamone-asr`), `Cargo.lock`(새 crate 한 묶음, 외부 crate 는 늘지 않았다), `crates/seamone-core/src/lib.rs`(`mod transcript;` 와 `pub use transcript::TranscriptSegment;`)
- 남은 곳(이번 범위 밖): 엔진 feature 와 구현(7-2), 측정 CLI 와 `make bench-asr`(7-3 나머지), 측정과 결과 문서(7-4, 7-5), `README.md` 2-2 의 받아쓰기 엔진 줄(7-6)

## 5. 방안과 판정
(엔진 후보를 2026-11-19 에 crates.io API, GitHub API, Codeberg API, 각 저장소의 README 와 BUILDING.md 로 조회해 견줬다, 이 노트북에서 빌드하거나 실행해 확인한 것은 없다, "확인하지 못함" 은 조회한 글에서 찾지 못했다는 뜻이다)
### 5-1. whisper.cpp 를 감싼 `whisper-rs`
- 판: whisper-rs 0.16.0, 그 안의 `whisper-rs-sys` 0.15.0
- 마지막 갱신: crates.io 2026-03-12, 저장소(https://codeberg.org/tazz4843/whisper-rs) 2026-03-15, 보관 처리 아님, 별 68개, 원본 whisper.cpp(ggml-org)는 2026-11-18 에 push 됐고 별 55,300개, 래퍼 저장소 master 가 묶은 whisper.cpp 는 v1.8.3(2026-01-15)이고 원본 최신 릴리스는 v1.9.5(2026-10-06)다
- 라이선스: Unlicense(래퍼), 원본 whisper.cpp 는 MIT
- 가속기: feature `metal`(Apple GPU), `coreml`(crates.io feature 목록에만 있고 README 설명은 없음), `cuda`, `hipblas`, `vulkan`, `openblas`, CPU SIMD 는 whisper.cpp 가 맡는다(원본 README: ARM NEON, Accelerate, Metal, Core ML), Android 와 iOS 는 래퍼 README 가 "Windows, macOS, Linux 밖은 시험할 방법이 없다" 고 밝혀 래퍼로는 확인되지 않았다, 원본 whisper.cpp 에는 iOS(`examples/whisper.objc`, `whisper.swiftui`)와 Android(`examples/whisper.android`) 예제가 있다
- 다섯 플랫폼 빌드: C++ 를 빌드한다, `whisper-rs-sys` 의 build 의존성이 `cmake`, `bindgen`(libclang 필요), C++ 컴파일러다, macOS M1 은 BUILDING.md 가 `.cargo/config.toml` 에 `rustflags = "-lc++ -l framework=Accelerate"` 와 cmake 를 요구하고, Windows 는 MSVC, LLVM(clang), cmake 를, Linux 는 "그냥 된다" 고 적었다, Android 와 iOS 빌드 안내는 없다
- 순수 Rust: 아니다(C++ 를 cmake 로 빌드)
- 장점: Whisper 계열(base, small, large-v3-turbo, large-v3)을 Apple GPU 로 돌리는 가장 널리 쓰이는 경로고 원본이 활발하다
- 단점: C++ 빌드라 다섯 플랫폼 CI 에서 무겁고 Android 와 iOS 는 래퍼가 확인하지 않았다, 래퍼의 갱신이 8개월 전이고 묶은 whisper.cpp 가 최신 릴리스보다 한 minor 판(v1.8.3 대 v1.9.5) 뒤다
- 판정: 채택(가속기 엔진, 잠정) -> 5-9
### 5-2. Candle 의 Whisper
- 판: candle-core 와 candle-transformers 0.11.1, Whisper 는 `candle-transformers` 의 `models::whisper`(`audio.rs`, `model.rs`, `quantized_model.rs`)
- 마지막 갱신: crates.io 2026-10-31, 저장소(https://github.com/huggingface/candle) 2026-11-18 push, 별 21,350개, 열린 이슈 997개
- 라이선스: crates.io 는 MIT OR Apache-2.0, GitHub 표기는 Apache-2.0
- 모델과 기능: 예제 README 가 언어 감지와 `--language` 고정, `--timestamps`, 모델 tiny 부터 large-v3 와 distil-large-v3 를 적었다, large-v3-turbo 를 받는지는 확인하지 못함
- 가속기: feature `metal`(Apple GPU, `candle-metal-kernels` 와 `objc2-metal`), `accelerate`(Apple CPU BLAS), `mkl`, `cuda`, Android 와 iOS 가속기는 확인하지 못함
- 다섯 플랫폼 빌드: C++ 나 cmake 가 필요 없는 Rust 의존(`gemm`, `rayon`, `safetensors`)이고 `candle-wasm-examples/whisper` 가 있어 브라우저 WASM 으로도 돈다, `tokenizers` 는 workspace 가 `default-features = false` 로 받고 `candle-core` 가 `fancy-regex`(순수 Rust)를 켜서 C 정규식(onig)은 들지 않는 것으로 읽힌다, `candle-metal-kernels` 에는 `build.rs` 가 없어(디렉터리 목록으로 확인) 빌드 때 Xcode 의 `metal` 컴파일러를 부르지 않는 것으로 추정하지만 확인하지 못함
- 순수 Rust: 기본 CPU 경로는 그렇다
- 장점: C++ 없이 Whisper 를 Apple GPU 로 돌릴 수 있는 후보라 다섯 플랫폼 빌드가 가장 단순할 것으로 추측한다(확인 전)
- 단점: Whisper 의 속도와 정확도 최적화가 whisper.cpp 만큼 다듬어졌는지 확인하지 못함, 열린 이슈가 많다
- 판정: 대안으로 보류(`whisper-rs` 가 모바일이나 CI 빌드에서 막힐 때) -> 5-9
### 5-3. `qwen-asr`
- 판: qwen-asr 0.11.4(CLI 는 별도 crate `qwen-asr-cli`)
- 마지막 갱신: crates.io 2026-11-12, 저장소(https://github.com/huanglizhuo/QwenASR) 2026-11-12 push, 별 66개, 열린 이슈 3개, 내려받은 수 2,419회(작고 새 프로젝트)
- 라이선스: MIT
- 모델: Qwen3-ASR 0.6B 와 1.7B, 오프라인과 streaming, 강제 정렬, `--language` 로 고정하거나 기본 자동 감지
- 가속기: CPU 전용이다(README 가 "GPU dispatch 없음" 이라 적었다), 손으로 짠 NEON, SME, Accelerate, AMX 커널, feature `blas`(기본, macOS 는 Accelerate, Linux 는 OpenBLAS), `vdsp`, `int8-encoder`, `int8-prefill`, 모바일은 feature `ios`(정적 라이브러리와 C-FFI, `cargo build --release --target aarch64-apple-ios --features ios`)와 `android`(JNI 공유 라이브러리, `cargo ndk -t arm64-v8a build --release --features android`)가 README 에 있다
- 다섯 플랫폼 빌드: 의존 crate 가 `libc` 하나고 cmake 나 C++ 가 필요 없다(`--no-default-features` 면 BLAS 링크도 없다), 기본 feature `blas` 는 Linux 에서 OpenBLAS 를 요구한다, README 의 빌드 안내는 macOS, Linux, iOS, Android 이고 Windows 는 적혀 있지 않다, 프로젝트 CI(`.github/workflows/ci.yml`)는 macOS 와 Ubuntu 만 돌리고 릴리스 바이너리도 aarch64-apple-darwin 과 x86_64-unknown-linux-gnu 둘뿐이라 Windows, Android, iOS 빌드는 프로젝트 CI 로 확인되지 않았다
- 순수 Rust: 그렇다
- 프로젝트가 밝힌 속도(우리가 잰 것이 아님): Apple M5 Pro 에서 0.6B 가 28.16초 음성을 추론 529ms 에 받아썼다(53배 실시간, 10번 가운데값), 이 노트북은 M1 이라 같은 수치를 기대할 수 없다
- 장점: Qwen3-ASR 를 받는 후보가 이것뿐이고 순수 Rust 에 모바일 빌드 안내가 있다
- 단점: 가속기(Apple GPU, 폰 GPU 와 NPU)를 쓰지 않는다(프로젝트 PD7 은 가속기를 쓴다), 작은 프로젝트라 유지 상태를 믿기 이르다, Windows 는 확인하지 못함
- 판정: 채택(순수 Rust 엔진, 잠정) -> 5-9
### 5-4. 그 밖에 찾았지만 이번에 평가하지 않은 것
- `sherpa-onnx` 2.0.1(k2-fsa, crates.io 2026-10-27, 내려받은 수 873,278회), `sherpa-rs` 0.6.8(2025-10-05), `ort` 2.0.0-rc.15(시험판, 2026-10-19), `transcribe-rs` 0.3.12(2026-10-02), 판과 갱신일만 조회했고 라이선스와 가속기와 플랫폼은 보지 않았다
### 5-9. 최종 방안
- 엔진 둘을 cargo feature 뒤에 구현한다: 가속기 엔진은 `whisper-rs`(feature `metal`), 순수 Rust 엔진은 `qwen-asr`, 근거와 상태는 `issue_decision.md` D8(결정자 worker, 잠정, 사용자 확인 필요 없음)
- 이유: 후보 모델(3-1 R3)이 Whisper 계열과 Qwen3-ASR 계열이고 Qwen3-ASR 를 받는 후보가 `qwen-asr` 뿐이며, Whisper 를 Apple GPU 로 돌리는 후보 가운데 `whisper-rs`(whisper.cpp)가 가장 널리 쓰이고 원본이 활발하다
- 기본 feature 에는 엔진을 넣지 않는다(2-3), `whisper-rs` 는 C++ 빌드라 feature 뒤에 두고 `qwen-asr` 도 Windows 빌드를 확인하기 전까지 기본에서 뺀다
- 갈림길: `whisper-rs` 가 Android, iOS 나 CI 의 C++ 빌드에서 막히면 Candle 의 Whisper 로 바꾼다
- 구현 전에 확인할 것(이번에 못 함): 두 crate 의 실제 빌드(이 노트북과 CI 다섯 플랫폼), 모델 파일 크기와 메모리, M1 에서 속도, 한국어 오류율은 #2 결과

## 6. 사용자 승인
- 해당 없음: 방향은 PD5, PD7 로 정해졌다, 시작 모델을 고르는 일은 프로젝트 PD9 에서 한다

## 7. 할 일과 검증
(순서대로 한다, 시간 한도: 모델 하나의 측정이 15분을 넘으면 음성을 5개로 줄이고 줄인 수를 적는다)
- [x] 7-1. 조각 타입과 interface: skill `contract-spec` 을 읽고 `crates/seamone-core` 에 전사 조각 타입을, `crates/seamone-asr` 에 `Transcriber` interface 와 시험용 가짜 엔진을 만든다 / 검증: 가짜 엔진으로 interface 계약의 단위 시험
  - 결과: `crates/seamone-core/src/transcript.rs:7`(전사 조각 `TranscriptSegment`: `start_ms`, `end_ms`, `text`, `speaker`)와 `:21`(`new`, 화자 칸을 비운 조각), `crates/seamone-core/src/lib.rs` 의 `mod transcript;` 와 `pub use`, `crates/seamone-asr/src/lib.rs:33`(`Transcriber`, 계약은 doc comment 에 적었다), `:44`(`TranscribeError`: `ModelUnavailable`, `InvalidInput`, `Failed`), `:67`(`check_input`), `:86`(`audio_ms`), `crates/seamone-asr/src/fake.rs:12`(`FakeTranscriber`)
  - 지시한 계약: 입력과 출력 타입은 D1, D2, 오류 타입이 모델을 못 불러온 경우와 입력이 잘못된 경우를 가르는 것은 D4(입력 검사가 먼저), 언어 힌트가 없을 때는 엔진이 언어를 스스로 가리는 것이 D3, 한 번에 하나의 일만 하는 메서드 하나짜리 interface 로 두고 스트리밍 같은 스펙에 없는 쓰임은 넣지 않았다
  - 검증: `make test`(08:03, `CARGO_BUILD_JOBS=1 nice -n 10`, EXIT=0)가 `seamone-asr` 20개(interface 와 가짜 엔진 12개, 7-3 의 순수 함수 8개)와 `seamone-core` 6개(전사 조각 3개, 앞 이슈 3개)를 통과했다, 가짜 엔진 시험은 돌려받은 결과만 확인한다(호출 횟수를 기대값으로 걸지 않는다), `make lint`(08:02, EXIT=0, 경고 0건)
- [ ] 7-2. 엔진 고르기: 2-3 의 엔진 후보를 crates.io 와 저장소에서 조회해 5-9 에 견줘 적고 둘 이상을 cargo feature 뒤에 구현한다(가속기를 쓰는 것 하나, 순수 Rust 하나) / 검증: 엔진마다 한국어 음성 하나를 받아써 글이 나온다
  - 이번에 한 것(조사만, 구현은 하지 않았다): 후보 셋(`whisper-rs`, Candle 의 Whisper, `qwen-asr`)과 그 밖의 넷을 조회해 5절에 견줘 적었다, 잠정 선택은 D8(가속기 `whisper-rs`, 순수 Rust `qwen-asr`)
  - 남은 것: 두 엔진의 구현과 한국어 음성 하나로 받아쓰기 확인(C++ 빌드와 모델 내려받기가 필요해 이번에 하지 않았다)
- [ ] 7-3. 측정 CLI 와 make target: 실시간 배수 계산은 순수 함수로 두고 단위 시험을 단다, 음성은 #2 의 표본에서 한국어 10개를 wav 로 꺼내 `.tmp/` 에 둔다 / 검증: `make test`, 가짜 엔진으로 CLI 가 끝까지 돈다
  - 이번에 한 것(순수 함수만): `crates/seamone-asr/src/speed.rs:11`(`audio_secs`), `:18`(`realtime_factor`), `:28`(`median`)와 단위 시험 8개(정상값, 처리 시간 0 이하, 음수, 숫자가 아닌 값, 홀수와 짝수 개수, 빈 목록, 입력 순서와 무관), 결정 D7
  - 남은 것: 측정 CLI 와 `make bench-asr`, 한국어 음성 10개를 wav 로 꺼내기(#2 의 표본이 필요), 가짜 엔진으로 CLI 가 끝까지 도는지
- [ ] 7-4. 측정: 모델 파일을 `.tmp/` 에 받아 작은 것부터 재고(CPU 만, Apple GPU), 한 모델이 끝나면 그 파일을 지운다 / 검증: 결과가 `.tmp/bench/` 에 남고 결과 문서로 옮겨진다
- [ ] 7-5. 결과 문서와 추천: 측정 조건(기기, 스레드 수, 양자화, 엔진 판), 모델마다 결과, 한계, #2 의 오류율과 합친 계층별 시작 모델 추천을 쓴다, 표를 쓰지 않고 목록으로 쓴다
- [ ] 7-6. `README.md` 2-2 의 받아쓰기 엔진 줄을 "있음" 으로 고치고 interface 와 조각 타입을 한 줄씩 적는다, 8절 수정 요약과 보고를 쓴다

### 7-7. GitHub 이슈에서 이어서 할 작업
- 게시본: [GitHub 이슈 #3](https://github.com/guny524/seamone/issues/3)

#### 목표와 범위
같은 한국어 음성으로 데스크톱 받아쓰기 엔진의 CPU·Apple GPU 속도를 비교하고, #2의 문자 오류율(CER)과 합쳐 시작 모델을 추천한다. 앱 연동은 #5에서 진행한다. 모바일 속도는 기기와 엔진이 준비된 뒤 측정한다.

- [x] 공통 `Transcriber` interface, 가짜 엔진과 실시간 배수 계산을 초안에 넣고 CI를 통과시킨다. [엔진 interface 초안 PR #8](https://github.com/guny524/seamone/pull/8), [CI](https://github.com/guny524/seamone/actions/runs/44428752393)
- [ ] `develop`에 들어간 `Transcriber` interface와 앱 호출부의 타입을 대조한다. 초안의 공통 타입과 가짜 엔진은 녹음·빠른 검수 병합에 포함됐다. [녹음과 기기 안 받아쓰기, 빠른 검수·교정 화면 PR (#10)](https://github.com/guny524/seamone/pull/10)
- [ ] Whisper.cpp 계열과 순수 Rust 엔진을 Cargo feature 뒤에 연결한다. 같은 한국어 WAV를 두 엔진에 넣어 전사와 구간 시간이 반환되는지 확인한다
- [ ] 고정한 한국어 샘플과 모델 목록으로 속도 측정 명령을 만든다. 실시간 배수 계산을 단위 시험으로 확인하고 가짜 엔진으로 측정 명령을 끝까지 실행한다
- [ ] M1에서 같은 입력으로 CPU와 지원되는 Apple GPU의 속도를 잰다. 모델, 양자화 방식, 엔진 버전, 스레드 수, 음성 길이, 실행 시간과 실시간 배수를 기록한다. 가속기를 쓸 수 없는 조합은 이유를 적는다
- [ ] #2의 CER와 속도를 대조해 데스크톱 시작 모델과 근거를 적는다. 모바일 실측이 없는 조합과 #5의 검증용 Whisper small을 표시한다
- [ ] `make lint`, `make test` 결과와 측정 재현 명령, 결과 문서, PR 링크를 이슈에 남긴다

#### 작업 위치와 선행 조건
`develop`의 `crates/seamone-asr/`에서 시작한다. #1의 앱 뼈대는 main에 병합됐고, #2의 오류율 결과는 `research/results/`에 있다. 모델 파일과 샘플 원본은 Git에 추가하지 않는다.

## 8. 수정 요약
(7-1, 7-2 의 조사, 7-3 의 순수 함수까지의 파일이다, 이 worktree 는 앱 뼈대 브랜치 위라 아래 파일을 stage 하지 않았고 메인이 경로로 골라 별도 브랜치로 커밋한다)
- `crates/seamone-core/src/transcript.rs`(새 파일): 전사 조각 `TranscriptSegment` 와 `new`, 단위 시험 3개
- `crates/seamone-core/src/lib.rs`(고침): `mod transcript;` 와 `pub use transcript::TranscriptSegment;` 를 더했다
- `crates/seamone-asr/Cargo.toml`(새 파일): 새 crate, 의존은 `seamone-core` 하나
- `crates/seamone-asr/src/lib.rs`(새 파일): `Transcriber`, `TranscribeError`, `check_input`, `audio_ms`, `SAMPLE_RATE_HZ`, interface 단위 시험 12개
- `crates/seamone-asr/src/fake.rs`(새 파일): 시험용 가짜 엔진 `FakeTranscriber`
- `crates/seamone-asr/src/speed.rs`(새 파일): 순수 함수 `audio_secs`, `realtime_factor`, `median` 과 단위 시험 8개
- `Cargo.toml`(고침): workspace 멤버에 `crates/seamone-asr`
- `Cargo.lock`(고침): 새 crate 한 묶음, 외부 crate 는 늘지 않았다

## 9. 지금 상태
- 공통 interface, 가짜 엔진과 속도 계산 커밋 `e763890`은 녹음·빠른 검수 병합을 통해 `develop`에 포함됐다
- 남은 것: 초안 검토, 실제 엔진 연결, M1 속도 측정, 시작 모델 추천. 순서와 완료 증거는 7-7절을 따른다

## 10. Closing
### 10-1. 검토
- TODO
### 10-2. 커밋 직전 증거 대조
- TODO
### 10-3. merge 직전 재확인
- TODO
### 10-4. 사람 시험
- 해당 없음: 측정과 시험은 명령으로 확인한다
### 10-5. 자원
- 공통 interface 작업 사본과 로컬 브랜치를 정리했다. 실제 엔진과 측정은 `develop`에서 시작한다
### 10-6. 이슈 마무리 코멘트
- 상태: TODO
### 10-7. 끝내기 승인
- TODO
### 10-8. 커밋과 merge
- TODO

## 11. 참고
- whisper.cpp https://github.com/ggml-org/whisper.cpp, CPU SIMD 와 Apple GPU 를 쓰는 Whisper 엔진
- Candle https://github.com/huggingface/candle, Rust 로 짠 ML 엔진과 Whisper 구현
- `qwen-asr` crate https://lib.rs/crates/qwen-asr, 순수 Rust 로 짠 Qwen3-ASR CPU 엔진
