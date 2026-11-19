# 이슈 결정
(형식은 `project-issue-conductor` skill 의 `references/record-format.md` 6절, 지우지 않고 쌓는다)

- D1 2026-11-19 전사 조각 `TranscriptSegment` 는 시각을 밀리초 정수(`start_ms`, `end_ms`)로, 글(`text`)과 화자 칸(`speaker: Option<String>`, 지금은 늘 `None`)을 가지고 `seamone-core` 의 `transcript` 모듈에 둔다, 이유: 정수 밀리초는 JSON 으로 화면(WASM)에 그대로 가고 부동소수 오차가 없다, 화자 칸만 두는 것은 프로젝트 PD10 이다, worker
- D2 2026-11-19 `Transcriber` 는 메서드 하나 `transcribe(&mut self, audio: &[f32], language: Option<&str>) -> Result<Vec<TranscriptSegment>, TranscribeError>` 로 둔다, 입력은 16kHz mono 표본(-1.0 에서 1.0 의 `f32`)과 언어 힌트 코드(`ko`, `en`), 이유: board 2-3 의 계약을 그대로 옮겼고 엔진이 추론 상태를 바꾸므로 `&mut self` 다, 스펙에 없는 쓰임(스트리밍, 진행 알림)은 넣지 않았다, worker
- D3 2026-11-19 언어 힌트가 없으면 엔진이 음성에서 언어를 스스로 가려 받아쓴다, 이유: Whisper 의 candle 예제(`--language` 를 안 주면 감지)와 `qwen-asr`(`--language` 기본 auto)가 모두 그렇게 동작한다, worker
- D4 2026-11-19 오류 타입 `TranscribeError` 는 `ModelUnavailable`(모델을 못 불러옴), `InvalidInput`(입력이 계약에 맞지 않음), `Failed`(받아쓰는 도중 엔진 실패) 셋이고 입력 검사가 모델 불러오기보다 먼저다, 이유: 호출하는 쪽이 고칠 수 있는 입력 문제와 엔진 문제를 가르라는 지시를 따랐다, `Failed` 는 지시한 둘에 들지 않는 추론 중 실패를 둘 가운데 하나로 잘못 분류하지 않으려고 더했다(없애려면 `Failed` 를 지우면 된다), worker
- D5 2026-11-19 입력 검사(빈 음성, 숫자가 아닌 표본, 빈 언어 힌트)는 공용 함수 `check_input` 하나에 두고 모든 엔진이 먼저 부른다, 엔진이 받지 못하는 언어 힌트는 엔진이 `InvalidInput` 으로 거절한다, 이유: 입력 계약을 한 곳에 두어 엔진마다 다르게 해석하지 않게 한다, 받는 언어 목록은 엔진마다 달라 공용 함수가 알 수 없다, worker
- D6 2026-11-19 `seamone-asr` 는 `seamone-core` 만 의존하고 시험용 가짜 엔진 `FakeTranscriber` 를 기본 feature 로 공개한다, 이유: 측정 CLI 의 연습 실행(가짜 엔진으로 끝까지 돈다)이 같은 가짜 엔진을 쓰고, 엔진 crate 는 이번에 들이지 않는다, worker
- D7 2026-11-19 실시간 배수와 가운데값은 `seamone-asr::speed` 의 순수 함수(`audio_secs`, `realtime_factor`, `median`)로 두고 뜻이 없는 입력(처리 시간 0 이하, 음수, 숫자가 아닌 값, 빈 목록)은 `None` 을 돌려준다, 이유: 측정 CLI 가 `None` 을 "재지 못함" 으로 적을 수 있다, worker
- D8 2026-11-19 엔진 후보는 잠정으로 가속기 엔진 `whisper-rs`(feature `metal`)와 순수 Rust 엔진 `qwen-asr` 를 먼저 구현하고, Candle 의 Whisper 는 `whisper-rs` 가 모바일이나 CI 빌드에서 막힐 때의 대안으로 둔다, 이유: 후보 모델(board 3-1 R3)이 Whisper 계열과 Qwen3-ASR 계열이고 Qwen3-ASR 를 받는 엔진 후보가 `qwen-asr` 뿐이다, 근거와 확인하지 못한 것은 board 5절, 상태 잠정(허용한 작업: 조사와 구현 준비, 멈출 단계: 두 crate 를 이 노트북과 CI 에서 실제로 빌드해 보기 전), worker
