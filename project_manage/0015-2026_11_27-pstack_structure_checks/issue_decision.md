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
