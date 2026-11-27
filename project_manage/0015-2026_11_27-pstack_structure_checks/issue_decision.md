# LLM 실수를 lint, build, compile 에서 거르게 폴더 구조와 검사를 바꾼다 decision

## D1. 이번 밤의 범위는 native 작업 crate 분리와 그 분리를 지키는 검사다 [밤사이 결정]
- 상태: 확정(메인, 2026-11-27 01:25), 사용자 확인 전
- 정한 것
  - 새 crate `crates/seamone-runtime` 에 저장소(store, schema SQL), 음성(audio, audio_ingest, normalize), 녹음(capture), 학습 대기열(training), 받아쓰기(inference)와 그 시험, fixture 를 `git mv` 로 옮긴다, `AppState` 의 메서드는 `LibraryService` 한 주인으로 모은다, host(`app/src-tauri`)에는 창 검사, dialog, mobile URI, Tauri 연결만 남긴다
  - 지키는 검사: `scripts/check_crate_boundaries.py` 의 허용 방향(host -> core, runtime / runtime -> core, asr)과 어댑터 주인(rusqlite, cpal, symphonia, hound, rtrb, objc2 계열은 runtime, tauri 계열은 host), runtime 내부를 crate 밖에서 import 하는 `compile_fail` doctest 와 통과하는 짝 doctest, 녹음 지원 플랫폼 정의가 host 와 runtime 에서 같은지 보는 시험, `desktop-whisper` feature 가 runtime 까지 넘어가는지 보는 시험, `lint-native` 와 `desktop-proof` target 의 대상 crate
  - 완료 조건에 PR 의 다섯 플랫폼 패키지 빌드(build.yml 산출물 6개) 통과를 넣는다
  - 그다음 local LLM 실수 유도 시험, 시간이 남으면 research 의 `shutil.rmtree` 직접 호출을 막는 AST lint
- 미룬 것과 까닭
  - typed IPC(명령마다 요청과 응답 타입 묶기): 이 검사가 막을 과거 실수가 기록에 없다(기록된 IPC 실수는 이름 오타와 등록 누락뿐이고 PR #12 의 catalog 가 이미 막는다), pstack `correct` 의 "새 검사는 실제 과거 실수에서 실패함을 증명" 조건에 못 미친다
  - UI 상태 controller 분리: typed IPC, PR #11 과 같은 파일(`app/ui/src/workspace.rs`, `capture.rs`)을 고쳐 한 묶음으로 다음 단계에 둔다
  - research 의 storage 와 comparison 분리: native 분리와 따로 움직이고, PR #13(base main), #14 와 같은 파일을 고친다
  - 파일 이름 바꾸기(`commands.rs` -> `ipc/handlers.rs` 등)와 mobile make target: 어느 실수 부류와도 이어지지 않고 diff 만 키운다
- 근거: worker 조사 보고(이슈 log 2026-11-27 01:14), Opus 5.5 high 설계 검토(세션 `107ac815-bac9-40af-a5d6-1e4a0c2cc762`), 5-1절 안(한 crate 안 module 나누기)을 버린 까닭은 crate 단위 의존 차단과 crate 경로의 compile_fail doctest 를 쓸 수 있기 때문이다(중첩 private module 도 형제 접근은 막으므로 "module 로는 못 막는다" 는 까닭이 아니다)
- 열린 PR: 이번 분리는 PR #11(app/ui), #13, #14(research)의 파일과 겹치지 않는다, 세 PR 은 분리가 develop 에 들어간 뒤 rebase 하고 검사를 다시 돌린다(사용자 답 "worker 가 새 구조로 옮김")

## D2. 기존 lint 예외는 새로 더하는 것만 거절하고 기존 것은 사람 결정을 기다린다
- 상태: 확정(메인, 2026-11-27 01:25), 기존 예외의 승인 방식은 사용자 결정 대기
- 정한 것: 이유(reason)는 이미 `Cargo.toml` 의 clippy `allow_attributes_without_reason` deny 로 강제된다, 만료일과 사람 승인은 새로 필요하다, 밤에는 기존 예외 10곳과 설정의 allow 3곳에 AI 가 승인이나 만료일을 지어 넣지 않는다, 책임을 나누면 사라지는 예외(`commands.rs:223` 의 mobile 전용 인자)는 없앤다, 검사는 새 예외를 거절하는 데까지만 만들고 완료라고 보고하지 않는다
- 근거: 사용자 답 "밤에는 새 예외 금지", pstack `correct` 의 "이미 흔한 패턴이면 더할 때만 실패시킨다"

## D3. PR #16 을 먼저 merge 하고 남은 우회는 새 PR 에서 고친다 [밤사이 결정]
- 상태: 확정(메인, 2026-11-27 04:03), 사용자 확인 전
- 정한 것: Opus 6~8단계 검토의 9건은 이번 PR 의 회귀가 아니라 아직 막지 못한 우회다, PR #16 은 CI(SQLite 판 의존 고침) 와 패키지 빌드가 통과하면 develop 에 merge 한다, 9건은 develop 에서 시작한 `guny524/15-guard-integrity` 의 새 PR 에서 고친다, 그 PR 은 검사와 규칙 파일이 바뀌면 실패하는 job 을 처음 넣으므로 스스로 그 job 에 걸리고 사람이 확인한 뒤 merge 한다
- 근거: 동작 변화와 약해진 시험이 없다는 Opus 검토(세션 `2733a86a`), 막을 때마다 다음 우회가 드러나는 흐름에서 한 PR 에 계속 쌓으면 merge 와 rebase 가 미뤄져 열린 PR #11, #13, #14 의 rebase 도 밀린다

## D4. 사람 확인 관문의 확인자는 메인이다, branch protection 과 기준 목록도 메인이 pstack 기준으로 정한다
- 상태: 확정(사용자, 2026-11-27 아침)
- 사용자 원문: "pstack 에서 말하는 주장하는 철학을 따라가, 내가 결정할 건 없어 pstack 에서 말하는 내용에만 안 반 하면 그대로 해", "너가 알아서 해 내가 확인할거면 pstack 이나 이런걸 끌고 오려고 하지도 않았겠지"
- 정한 것: 검사와 규칙 파일을 고친 PR, 기준 목록(`scripts/*.json`)을 늘리는 PR 의 "사람 확인" 은 메인이 독립 검토(다른 모델 검토)와 시험 대조를 근거로 하고 근거를 기록에 남긴다, branch protection 의 필수 check 는 메인이 정해 건다
- 근거: pstack `correct` 는 예외와 검사 변경에 사람 승인을 요구하지만, 사용자는 그 판단을 메인에게 맡겼다

## D5. lint 예외는 승인하지 않고 설계를 고쳐 없앤다, 주석은 pstack no-comments 대로 정리하고 금지한다
- 상태: 확정(사용자, 2026-11-27 아침)
- 사용자 원문: "왜 예외가 있는데? 너 그런 얘기 알지, single resposibility 원칙 SOLID 에서 지키지 못하면 그건 충분히 설계가 안 된거라고, lint 예외가 발생하면 그건 충분히 설계가 안 된거야", "우리 주석 금지한 거 맞지?"
- 사실: 주석 금지는 그때까지 정한 적이 없다(레포 기록과 지난 대화에 없음), Rust `//` 207줄, Python `#` 31줄이 있었다
- 정한 것: 기존 lint 예외 15곳(Rust `#[expect]` 11, Cargo allow 3, Ruff 시험용 ignore 1)은 만료일과 승인을 다는 대신 설계를 고쳐 모두 없애고 기준 목록을 비운다, 주석은 pstack `no-comments` 대로 바꿀 수 없는 외부 사정을 적은 것만 남기고, 제약을 적은 주석은 검사로 옮긴 뒤 지우고, 남은 주석이 늘면 lint 가 거절한다, 문서 주석(`///`, `//!`, docstring)은 doctest 와 공개 설명이라 남긴다

## D6. 재생 확인은 runtime 이 실제 재생을 소유해 근본부터 고친다
- 상태: 확정(사용자, 2026-11-27 아침), 설계는 메인이 정해 검토받는다
- 사용자 원문: "뭔 말이야? 왜 검수를 땜빵 칠려고 해? 땜빵 치지 말고 문제를 근본적으로 해결할 수 있게 해"
- 원인: 화면(웹뷰)이 음성을 재생하고 "다 들었다" 를 스스로 알리면 runtime 이 그 말을 믿는다, 그래서 재생하지 않고 끝 알림만 보내도 확인 전사가 된다
- 정한 방향: runtime 이 음성을 직접 장치로 내보내고 실제로 그 구간의 끝까지 재생된 것을 runtime 이 확인해 증거를 만든다, 화면은 재생을 조작하고 보여 주기만 하고 끝을 주장하지 않는다, 최소 시간 대기 같은 땜빵은 쓰지 않는다

## D7. "끝까지 들었다" 의 정의와 runtime 출력 설계 [사용자 위임 결정]
- 상태: 확정(메인, 2026-11-27 13:18), 사용자 원문 "땜빵 치지 말고 문제를 근본적으로 해결할 수 있게 해", "pstack 에서 말하는 주장하는 철학을 따라가, 내가 결정할 건 없어" 에 따라 메인이 정했다, 사용자가 다르게 정하면 바꾼다
- 근거: Opus 5.5 high 설계 자문(세션 `4a698cb5-a15d-4912-8bfb-da4b3e78fd3d`)
- 증거의 뜻: runtime 의 출력 callback 이 조각 구간의 모든 16 kHz source sample 을 장치 버퍼에 넘겼고, 장치 시계(`OutputCallbackInfo::timestamp` 의 playback 과 callback 시각)가 그 버퍼의 재생 끝을 지났다, 화면은 재생 조작과 위치 표시만 하고 끝을 주장하지 않는다, `playback_ended` IPC 는 없앤다, 벽시계로 기다리는 방식은 쓰지 않는다
- 정한 정책
  1. 들은 범위는 run 들의 합집합으로 센다, 멈춤과 되감기는 허용하고 앞으로 넘긴 빈 곳은 나중에 채워 들어도 된다
  2. 재생 중 장치 오류가 나면 그 session 의 들은 범위를 무효로 하고 처음부터 다시 듣는다
  3. 증거는 "앱이 장치로 끝까지 내보냈다" 이다, 사람이 실제로 들었는지와 OS 의 소리 끄기는 어떤 코드로도 증명할 수 없다, 앱 안에 볼륨 조절을 두지 않는다, `AGENTS.md` 의 검수 정책 문구를 이 뜻으로 고친다
  4. 배속은 1배만 둔다, 나중에 허용해도 source sample 로 센다
  5. 모바일 출력: 지금 모바일에는 받아쓰기가 없어 검수 화면이 열리지 않는다, 새 IPC 는 공통 catalog 에 두고 runtime 의 출력 trait 로 구조를 지금 잡는다, 모바일 구현(cpal 판 올리기와 Android AAudio, iOS audio session)은 모바일 받아쓰기 이슈에서 한다(rule change.md 7절)
- 단계: PR 1 은 증거 경로와 교정, 판정 화면과 `playback_ended` 없애기, PR 2 는 빠른 검수를 runtime 출력으로 옮기고 UI crate 의 web-sys feature 에서 `HtmlAudioElement`, `HtmlMediaElement` 를 빼 웹뷰가 소리를 내는 길을 컴파일 단계에서 없앤다
- 12단계(lint 예외와 주석)가 모든 파일을 건드리므로 그 뒤에 같은 worker 가 순서대로 한다
