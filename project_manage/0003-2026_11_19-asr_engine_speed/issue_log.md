# 이슈 log
(형식은 `project-issue-conductor` skill 의 `references/record-format.md` 7절, 시각 순으로 쌓고 지우지 않는다)

## 2026-11-19 08:20 worker(app, Sonnet 5.5 max) 보고
- 시작 지시는 이슈 폴더 `.tmp/start-1.md`(메인 08:00, 시간 한도 08:35, 무거운 일 없이), 보고 원문은 `.tmp/report-1.md`(git 에서 빠지는 임시 파일이라 요점을 아래에 남긴다), 파일은 앱 뼈대 worktree(`/Users/min-jo/orca/workspaces/seamone/1-app-skeleton-ci`)에 stage 하지 않고 뒀다
- 한 일: 7-1 전체(전사 조각 `TranscriptSegment`, `Transcriber` interface 와 오류 타입, 입력 검사, 시험용 가짜 엔진), 7-2 의 조사만(엔진 후보 셋과 그 밖의 넷을 조회해 board 5절에 견줌), 7-3 의 순수 함수만(실시간 배수, 가운데값, 음성 길이)
- 직접 확인한 것: `make lint`(08:02, `CARGO_BUILD_JOBS=1 nice -n 10`, EXIT=0, 경고 0건)와 `make test`(08:03, EXIT=0, `seamone-asr` 20개와 `seamone-core` 6개, 앞 이슈 시험 포함 모두 통과), 새 crate 는 외부 crate 를 늘리지 않았고(`Cargo.lock` 에 새 crate 한 묶음), 엔진 후보의 판, 갱신일, 라이선스, feature, 빌드 의존성은 crates.io API, GitHub API, Codeberg API, README 로 조회했다
- 못 한 것: 엔진 구현과 모델 내려받기, C++ 빌드, 측정 CLI 와 `make bench-asr`, 한국어 음성 꺼내기(#2 의 표본 필요), CI 확인(push 전), 엔진 후보를 실제로 빌드해 보는 일
- 결정: board 와 `issue_decision.md` D1 ~ D8, 엔진은 잠정으로 가속기 `whisper-rs`(Metal)와 순수 Rust `qwen-asr`(D8), Candle 의 Whisper 는 `whisper-rs` 가 막힐 때의 대안
- 엔진 후보 견줌 요약: `whisper-rs` 0.16.0(Unlicense, 갱신 2026-03, 묶은 whisper.cpp v1.8.3 대 원본 v1.9.5, Metal 이 feature, cmake 와 libclang 과 C++ 필요, Android 와 iOS 는 래퍼가 시험하지 않음), Candle 0.11.1(MIT OR Apache-2.0, 갱신 2026-10-31, Whisper 구현과 Metal feature, 순수 Rust 로 읽힘), `qwen-asr` 0.11.4(MIT, 갱신 2026-11-12, Qwen3-ASR 전용, CPU 전용, 의존이 `libc` 하나, iOS 와 Android 빌드 안내 있음, Windows 는 프로젝트 CI 에 없음, 작은 프로젝트)
- 다음에 할 일: (1) `whisper-rs` 를 feature 뒤에 넣어 이 노트북과 CI 에서 빌드되는지부터 본다, (2) `qwen-asr` 를 같은 방식으로, (3) 측정 CLI 와 `make bench-asr`, (4) #2 의 한국어 음성 10개가 오면 측정
