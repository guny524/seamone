# LLM 실수를 lint, build, compile 에서 거르게 폴더 구조와 검사를 바꾼다 log

## 2026-11-27 01:03 worker 1단계(조사와 설계) 시작
- Codex `gpt-6.1-sol` high worker 를 Orca worktree `/Users/min-jo/orca/workspaces/seamone/15-structure-checks`(브랜치 `guny524/15-structure-checks`, 기준 `2acdfb0`)에 띄웠다, handle `term_80ba4343-14c4-40db-9a12-24b2a7b7c5d0`
- 지시는 조사와 설계만, 코드와 폴더는 바꾸지 않는다

## 2026-11-27 01:14 worker 1단계 보고
- 실수 부류 여섯을 찾았다, 각각 두 번 이상 근거가 있다
  - 공유 cache 를 범위보다 넓게 지움(`65bb8c4` 의 `_drop_hub_cache`, FL 준비 기록)
  - 동시에 바꾸는 상태에 예약과 단일 소유가 없음(저장소 학습 조각 중복 처리, 연구 세트 준비 경합 `398e2fb`)
  - 파일의 완료 경계와 손상 상태를 잘못 다룸(JSONL 꼬리, 설정 쓰기 중단, PR #14 `9cfc9e1`)
  - 비교와 재개 자료의 동일성을 덜 확인함(모델 revision, 표본 목록, PR #13 정답 대조)
  - 확인 증거가 부족한 전사를 학습 정답으로 만들려 함(No 판정 뒤 같은 글 확정, 오류 표시 뒤 초안 재열기)
  - 여러 비동기 작업이 같은 화면 상태를 덮음(저장 안내 덮임, 조회 실패가 엔진 미지원으로 표시)
- 추천 구조: native 작업을 새 crate `seamone-runtime` 하나로 옮기고 저장소, 음성, 녹음, 검수, 학습 대기열을 비공개로 숨긴다, host(`app/src-tauri`)에는 Tauri 와 OS 파일 접근만 남긴다, IPC 는 명령마다 요청과 응답 타입을 묶는다, UI 는 상태 controller 와 화면을 나눈다, 연구 코드는 파일 쓰기와 cache 삭제를 `storage` package 로 모은다
- 메인이 원본에서 확인: `app/src-tauri/src/lib.rs:10` 의 `pub mod store`, `app/ui/src/api.rs:17` 의 `call<T>(Command, impl Serialize)`, `app/src-tauri/src/commands.rs:31` 의 공개 필드 `store`, `background`, `recorder`, host `Cargo.toml` 의 rusqlite, symphonia, cpal 직접 의존
- 기존 lint 예외 10곳(`#[expect]`, 숫자 변환과 mobile 전용 인자)은 이유만 있고 만료일과 사람 승인이 없다, 밤에는 AI 가 승인을 지어 넣지 않는다(사용자 답 "밤에는 새 예외 금지")
- 보고 원문은 `.tmp/conductor/15-structure/e24ad126fa139773.md`(버려도 되는 사본), Opus 5.5 high 설계 검토를 맡겼다

## 2026-11-27 01:50 runtime crate 분리 구현과 메인 검증
- worker 가 저장소, 스키마, 음성, 녹음, 받아쓰기, 학습 대기열을 `crates/seamone-runtime` 으로 `git mv` 하고 `LibraryService` 에 모았다, 스키마 SQL 4개와 fixture 4개는 내용이 그대로다(rename 100%)
- worker 의 과거 실수 되살리기: host 에서 `Store::mutate` 호출은 `E0624 method store is private`, host 에 rusqlite 는 `check-imports` 가 LibraryService 를 쓰라는 오류, `pub mod store` 는 compile_fail doctest 실패, 두 연결 중복 예약은 `two_connections_return_only_one_successful_claim` 실패(left 2, right 1), 새 `allow(dead_code)` 는 `check-lint-exceptions` 거절
- rustdoc 은 `compile_fail,E9999` 처럼 틀린 오류 번호를 넣어도 통과했다(Rust 1.99.1), 오류 번호 대조에 기대지 않고 통과하는 짝 doctest 와 실제 우회 코드의 E0624 로 대조한다
- 메인이 같은 worktree 에서 `make check-architecture`, `make lint`, `make test`(99개 통과, 실패 0)를 돌렸다, host `Cargo.toml` 에 hound 를 임시로 넣어 `check-imports` 가 "import hound in seamone-runtime; use seamone_runtime::LibraryService" 로 실패하는 것을 보고 되돌렸다
- CI 에 새로 넣은 `actions/setup-python` 고정 SHA `ece7cb06` 는 실제 v6.3.0 태그다
- `check-domain` 은 노트북 전역 Rust 에 `wasm32v1-none` 이 없어 실패했다, worker 가 레포 `.tmp/structure-checks/domain-sdk/` 에만 그 target 라이브러리를 받아 sysroot 로 통과시켰다, CI 의 domain job 은 `rustup target add` 로 받는다
- 커밋 `b2d2990`(runtime 분리와 검사), `ce55aac`(README 와 AGENTS.md), draft PR #16 https://github.com/guny524/seamone/pull/16

## 2026-11-27 01:58 Opus 구현 검토와 3단계
- Opus 5.5 high 구현 검토(세션 `98320cd9-874f-4c9b-8268-f404b4c8121a`): 옮긴 코드의 동작 변화는 없음, 결함 8건은 시험이 줄었거나 검사가 문서보다 좁게 막는 곳이다
  - 창 닫기 연결 시험과 main 창 IPC 성공 경로 시험이 줄었다
  - compile_fail doctest 가 경로 네 개만 막는다(`pub use store::Store` 로 피해 간다)
  - host 의존 검사가 금지 목록이라 `libsqlite3-sys` 같은 다른 crate 로 피해 간다
  - lint 억제 기준 목록을 같은 변경에서 늘려도 막지 않는다, 매개변수 속성과 lint 설정 끄기를 못 찾는다
  - 녹음 지원 플랫폼 대조가 Linux 시험에서만 돈다
  - 문서가 "증거 없이 검수 결과를 바꾸는 길을 막았다" 고 실제보다 넓게 말한다(재생을 듣지 않고 끝 알림을 보내는 길은 2acdfb0 전부터 남아 있다)
- 앞 일곱 건과 문구 좁히기를 같은 worker 에게 맡겼다, 최소 재생 시간 확인은 검수 동작을 바꿔 사용자 질문으로 올렸다(프로젝트 board 3절 물을 것)
