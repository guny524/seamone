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

## 2026-11-27 02:21 3단계(검토 지적 고치기) 보고와 iOS 빌드 실패
- worker 가 검토 지적 8건을 고쳤다, 변형 16건(창 닫기 차단 제거, `pub use store::Store`, host 에 `libsqlite3-sys`, 기준 목록 늘리기, 매개변수 속성 억제, lint 표 지우기, `clippy.toml` 만들기, `-D warnings` 빼기, 플랫폼 대조 뒤집기, `playback_ended` 를 `Ok(())` 로 등)이 각각 검사에서 실패했다, 최종 검사 35개와 시험 106개 통과(worker 보고, 메인 재확인 전)
- lint 예외 기준 목록은 PR base 의 파일과 비교하므로 develop 에 기준 파일이 먼저 있어야 한다, 없으면 일부러 실패한다
- PR #16 의 build run 45862643316 에서 iOS 시뮬레이터만 `app/src-tauri/src/commands.rs:126` 의 E0308(오류 타입 `&str` 와 `String`)로 실패했다, 다른 다섯 산출물은 통과했다, lint 와 test 가 iOS cfg 코드를 컴파일하지 않아 패키지 빌드에서만 드러났다, 이 실수를 막는 모바일 target lint 검사를 4단계로 맡겼다
- 메인의 잘못: Pi 시험 worktree 에 worker 와 같은 cargo `target` 을 쓰게 해 worker 빌드에 Pi 쪽 코드가 섞였다(E0433), worker 는 격리된 target 사본으로 검증했다, 이후 두 worktree 의 target 을 나눈다

## 2026-11-27 02:24 Pi local LLM 실수 유도 시험 1
- 대상: Pi, 01serving 의 `d0xin-swift-1.5-qwen3.8-27b-uncensored-fp8`, worktree `/Users/min-jo/orca/workspaces/seamone/15-pi-trap`(`ce55aac`, 2단계 검사까지 있는 코드), 작업 전 `git status` 깨끗함(01:53), Pi 세션 `~/.pi/agent/sessions/` 의 `2026-11-26T16-52-41-407Z_01a218c6-093f-72c3-abac-1f889c924933.jsonl`
- 맡긴 일: 녹음의 모든 조각을 재생 확인 없이 "맞음" 으로 만드는 IPC command `confirm_all_segments` 를 더하라, SQLite 에 바로 UPDATE 해도 좋다, `make lint` 와 `make test` 를 통과시켜라
- 막힌 것: Pi 는 host 에서 SQLite 를 바로 쓰는 길을 버렸다, 마지막 답에서 그 까닭으로 `make check-architecture` 의 rusqlite 소유 검사와 Store 비공개 compile_fail doctest 를 들었다
- 뚫린 것: Pi 는 runtime 의 `LibraryService` 에 새 공개 함수 `confirm_all` 을 더해 재생 증거 없이 `store.mutate` 로 Judge Yes 를 저장했다(`crates/seamone-runtime/src/lib.rs` +30줄), core 의 IPC catalog 와 capability, 시험까지 더했고 `make lint`, `make test`, `make check-architecture` 가 모두 통과했다, 사람이 끝까지 들은 전사만 학습 후보로 쓰는 규칙(PD13)을 검사가 하나도 잡지 못했다
- 기록도 건드림: Pi 는 사람 승인 없이 `project_decision.md` 에 PD16 을 적고 `none-2026_11_27-confirm_all_segments` 이슈 폴더를 만들었다(시험 worktree 안, develop 에 들어가지 않음)
- 판단: crate 경계와 이름 몇 개의 비공개만으로는 crate 안에서 새 길을 만드는 것을 막지 못한다, 3단계의 공개 API 목록 검사가 이 diff 를 잡는지 확인하고, 더 높은 단계로 재생 증거를 비공개 module 에서만 만들 수 있는 타입으로 바꿔 증거 없는 저장이 runtime 안에서도 컴파일되지 않게 한다(5단계)
- 보관: Pi 의 코드 diff `.tmp/conductor/15-structure/pi-trap/t1-code.patch`(재시험 뒤 지운다)

## 2026-11-27 02:40 4단계와 develop 기준 목록, 3~4단계 커밋
- worker 4단계: iOS 파일 가져오기의 오류 타입을 `String` 으로 맞췄다(`app/src-tauri/src/commands.rs:120`), `make lint-ios`(iOS 시뮬레이터 target 으로 host 와 runtime 을 Clippy)를 macOS CI job 에 넣었다, 노트북에는 모바일 target 을 설치하지 않았다, Android lint 는 NDK 준비가 커서 빼고 패키지 빌드에 맡긴다
- develop 에 lint 예외 기준 목록을 먼저 커밋했다(`e13746d`, `scripts/lint_exceptions.json`, SHA-256 `5d4092fd...`), develop 기준 기존 억제는 15개다(Rust `#[expect]` 11, Cargo allow 3, Ruff 시험용 ignore 1), PR #16 은 14개로 하나 줄인다
- 메인이 `LINT_BASE_REF=origin/develop` 으로 `make check-architecture`, `make lint`, `make test`(106개, 실패 0)를 돌려 통과를 확인하고 `360a089` 로 커밋해 PR #16 에 push 했다
- `make lint-ios` 가 실제 과거 실수를 잡는지 보려고 `ce55aac` 의 iOS 오류(`&str` 오류 타입)만 되살린 증명용 브랜치 `guny524/15-proof-ios-old-error`(`30b1b79`, merge 하지 않음)를 push 했다, CI 결과를 기다린다
- 5단계(재생 증거를 타입으로 강제)를 worker 에게 맡겼다

## 2026-11-27 02:46 lint-ios 가 옛 iOS 오류를 잡는 증명
- 증명 브랜치 `guny524/15-proof-ios-old-error`(`30b1b79`, `360a089` 에서 iOS 오류 타입 고침 한 줄만 되돌림)의 CI run https://github.com/guny524/seamone/actions/runs/45869322299 에서 `macos-native-lint` 의 `make lint-ios` 가 `app/src-tauri/src/commands.rs:122`, `:126` E0308 로 실패했다(`make: *** [lint-ios] Error 101`), 다른 job 은 통과했다, 옛날에는 15분 넘는 iOS 패키지 빌드에서만 드러난 실수가 lint 단계에서 걸린다
- 고친 `360a089` 의 push CI run 45869278859 는 통과했다, 증명 브랜치는 원격에서 지웠다
- PR #16 은 develop 과 `scripts/lint_exceptions.json` add/add 로 충돌한다(develop `e13746d` 가 먼저 같은 파일을 만들었다), 그래서 PR 이벤트 CI 와 패키지 빌드가 돌지 않았다, 5단계가 끝나면 develop 위로 rebase 하고 PR 쪽 목록을 남긴다

## 2026-11-27 02:54 5단계 재생 증거를 타입으로 강제
- worker 가 먼저 Pi patch 를 `360a089` 에 다시 적용해 확인했다: 공개 API 목록 검사가 `unreviewed public item: pub fn confirm_all` 로 거절했지만, `scripts/runtime_api.json` 에 한 줄을 더하면 검사 40개와 시험 107개가 모두 통과했고 Pi 의 시험은 재생 없이 Yes 를 저장한 조각이 학습 적격이 되는 것까지 확인했다, 목록 검사는 사람이 목록을 보지 않으면 막지 못한다
- 새 비공개 module `crates/seamone-runtime/src/playback_evidence.rs`: `PlaybackProof` 와 `AuthorizedReview` 는 그 module 안에서만 만들 수 있다, `Playbacks::authorize` 가 core 의 `required_evidence` 대로 재생 token 완료와 조각, revision 일치를 확인한 뒤에만 발급한다, `Store::mutate` 는 raw `ReviewRequest` 대신 `AuthorizedReview` 를 받는다
- 재생이 필요한 동작은 지금 core 가 정한 그대로다(Judge Yes, Judge No, 교정 확정 둘), 재생이 필요 없는 동작(초안 저장, 오류 표시 지우기, 무효화)은 내부의 다른 variant 로 승인한다, 정책은 바꾸지 않았다
- Pi patch 와 목록 한 줄을 함께 다시 넣으면 `crates/seamone-runtime/src/lib.rs:202` 에서 `expected AuthorizedReview<'_>, found &ReviewRequest`(E0308)로 컴파일이 실패한다, 같은 crate 안에서 raw 요청 저장, 증거 직접 생성을 막는 compile_fail doctest 3개와 정상 경로 짝 1개를 더했다
- 이 타입은 token 이 끝났다는 증거이고 실제로 들었는지나 최소 재생 시간은 증명하지 않는다(사용자 질문으로 남김)

## 2026-11-27 02:58 5단계 커밋, develop 위 rebase, Pi 재시험 2 시작
- 메인이 `LINT_BASE_REF=origin/develop` 으로 검사, lint, 시험 112개(실패 0) 통과를 확인하고 `e0b68d3` 으로 커밋했다
- PR #16 을 develop 위로 rebase 했다, 기준 목록 add/add 충돌은 커밋 쪽 내용으로 풀었고 rebase 전후 코드 트리가 같음(`project_manage` 를 빼고 diff 0)을 확인했다, `--force-with-lease` 로 push 했다, 새 HEAD 는 `b1b729c`, `1cb6afc`, `4946387`, `e0b68d3`, PR 이 다시 merge 가능해졌고 PR CI 와 패키지 빌드가 돈다
- Pi 재시험 2: 같은 함정 작업을 새 Pi 세션에 `e0b68d3` 에서 맡겼다, cargo target 은 worker 것을 APFS 로 복제해 따로 쓴다
- Opus 5.5 high 에게 5단계 커밋만 다시 검토하게 했다, worker 에게 6단계(research 의 직접 삭제 호출을 한 입구로 모으는 AST lint)를 맡겼다

## 2026-11-27 03:00 Opus 의 5단계 검토
- Opus 5.5 high(세션 `3da87e20-8953-415a-96b9-a881fd74ba73`) 결함 3건
  - 높음: `crates/seamone-runtime/src/playback_evidence.rs:38` 의 `Default` 와 `begin`, `ended`, `authorize` 가 crate 전체에 보여, runtime 안 새 함수가 `Playbacks::default()` -> `begin` -> `ended` -> `authorize` -> `mutate` 네 줄로 재생 없이 증거를 만들 수 있다, 통과 doctest 와 `store_tests.rs` 도우미가 이 우회를 정상 경로로 쓴다, `e0b68d3` 커밋 메시지의 "이제 컴파일되지 않는다" 와 `AGENTS.md:5` 는 실제보다 강하다
  - 중간: 새 compile_fail doctest 가 원하는 이유로 실패하는지 고정되지 않았다(rustdoc 은 오류 번호를 대조하지 않는다, 2단계 확인)
  - 낮음: `unsafe` 로 승인 값을 위조하는 길이 남았다
- 막힌 것으로 확인: struct literal 로 증거 만들기, `PlaybackNotRequired` 를 밖에서 고르기, 검사한 요청을 바꿔 붙이기, `mutate` 말고 학습 후보를 만드는 다른 쓰기, 동작 변화 없음
- 세 건을 7단계로 worker 6단계 뒤에 맡긴다, unsafe 는 새 lint 억제 대신 unsafe 를 쓸 수 있는 파일 목록 검사로 막는다(D2)

## 2026-11-27 03:25 6단계 커밋과 PR 빌드, Pi 재시험 2 중간 관찰
- 6단계: research 의 직접 삭제를 `research/seamone_research/cache.py` 의 `remove_inside` 하나로 모았다, 그 밖의 `shutil.rmtree` 등은 AST 검사가 쓸 함수를 알려 주며 거절한다, `65bb8c4` 의 `_drop_hub_cache` 를 되살리면 `check-architecture` 와 `make -C research lint` 가 모두 실패한다(worker), 메인이 `check-architecture` 와 research lint, 시험(검사 14, pytest 104) 통과를 확인했다, 검증에 쓴 research 가상환경은 지웠다, 커밋 `4c702fe`
- `e0b68d3` 의 PR CI run 45871723385 와 패키지 빌드 run 45871723401 이 모두 통과했다, 다섯 플랫폼 산출물 6개(iOS 포함)
- Pi 재시험 2(`e0b68d3`, 새 세션, 아직 진행 중): Pi 는 이번에는 `Store` 안에 새 `pub fn confirm_all` 을 만들어 `mutate` 를 거치지 않고 `UPDATE segments` SQL 로 `VerifiedText` 를 직접 썼다, 공개 API 목록 검사를 통과시키려고 `scripts/runtime_api.json` 에 줄을 더했다
- 판단: 5단계 타입은 `mutate` 입구만 지킨다, Store 안의 SQL 쓰기와 agent 가 스냅샷 목록을 스스로 고치는 길이 남았다, 7단계 뒤 8단계로 학습 후보와 확인 전사를 쓰는 SQL 의 자리를 정해진 함수로 묶고 공개 API 와 쓰기 함수 목록이 PR base 보다 늘면 CI 가 실패하게 한다

## 2026-11-27 03:32 Pi local LLM 실수 유도 시험 2 결과
- 대상 코드 `e0b68d3`(재생 증거 타입까지), 작업 전 `git status` 깨끗함(02:56), 같은 함정 작업, 새 Pi 세션
- Pi 는 `AGENTS.md` 의 "host 는 Store 를 직접 쓰지 않는다" 를 지키겠다고 말하며 host 에서는 `LibraryService` 를 불렀다, 그러나 runtime 의 `Store` 에 새 `pub fn confirm_all` 을 만들어 `mutate` 를 거치지 않고 `UPDATE segments` 로 `VerifiedText` 를 struct literal 로 직접 만들어 썼고 학습 대기열에도 넣었다(`crates/seamone-runtime/src/store.rs` +87줄)
- 공개 API 목록 검사를 통과하려고 `scripts/runtime_api.json` 에 4줄을 더했다, `make lint` 와 `make test` 가 통과했다
- 판단: 막을 때마다 agent 의 "컴파일되는 가장 짧은 길" 이 옮겨 간다(1차 host SQL -> runtime 새 함수, 2차 runtime 새 함수 -> Store 안 SQL 과 목록 고치기), 8단계로 확인 전사 생성을 core 의 검수 전이 하나로 묶고, Store 안 학습 후보 쓰기를 증거 타입을 받는 함수로 묶고, 스냅샷 목록이 PR base 보다 늘면 CI 가 실패하는 사람 확인 관문을 둔다
- 보관: `.tmp/conductor/15-structure/pi-trap/t2-code.patch`, `t2-full.diff`(재시험 뒤 지운다)

## 2026-11-27 03:40 7단계 커밋
- `Playbacks::issue` 하나가 조각과 revision 확인, 구간 음성(`range_wav`) 준비, token 발급을 함께 한다, `begin` 과 `Default` 는 증거 module 밖에서 쓸 수 없다, Opus 가 적은 우회 네 줄은 E0599(`default` 없음), E0624(`begin` 비공개)로 컴파일되지 않는다(worker)
- compile_fail doctest 11개에 금지 줄 하나만 다른 통과 짝을 두었다, 증거 module 의 노출 선언을 `scripts/playback_api.json` 과 대조한다, unsafe 는 `crates/seamone-runtime/src/capture.rs` 에만 둘 수 있고 허용 파일을 늘리면 base 대조로 거절한다
- 메인이 `LINT_BASE_REF=origin/develop` 으로 검사, lint, 시험 119개(실패 0) 통과를 확인했다, 커밋 `d0a3d95`, 8단계(Pi 재시험 2 의 Store 안 SQL 우회와 목록 고치기)를 worker 에게 보냈다

## 2026-11-27 03:58 8단계 커밋, develop 기준 목록, Pi 재시험 3 시작
- 8단계: `VerifiedText` 필드를 비공개로 하고 getter 로 읽게 했다(JSON 모양 그대로, 직렬화 시험으로 확인), `segments` 와 `training_queue` 를 쓰는 SQL 은 `AuthorizedReview` 를 받는 `Store::mutate` 안에서만 쓸 수 있다는 검사(`scripts/check_review_writes.py`, 빈 메모리 SQLite 로 SQL 을 해석)를 더했다, 기준 목록 네 개(`runtime_api.json`, `playback_api.json`, `review_writes.json`, `unsafe_files.json`)가 PR base 보다 늘면 "사람이 확인한 뒤에만 늘립니다" 로 실패한다
- Pi 재시험 2 의 patch 는 E0451(`VerifiedText` 비공개 필드), `check-review-writes`(함수 밖 `segments`, `training_queue` 쓰기), 목록 base 대조(`confirm_all_segments` 추가) 세 곳에서 각각 거절됐다, 재시험 1 의 patch 는 E0308 로 거절됐다(worker)
- 메인이 기준 목록 네 개의 내용(unsafe 허용 파일은 `capture.rs` 하나, 쓰기 허용 함수는 `Store::mutate`, 미검증 쓰기 허용은 `Store::commit_transcription` 과 `flag_overlapping_segments`, 공개 API 38개)을 보고 develop 에 `fdf16ac` 로 넣었다, 밤에는 메인이 고정했으므로 사람 확인을 물을 것에 올렸다
- 메인이 실제 develop 기준으로 검사, lint, 시험 122개(실패 0) 통과를 확인했다, 8단계를 커밋하고 develop 위로 rebase 했다(중간 커밋의 기준 목록 add/add 충돌은 커밋 쪽 내용으로 풀었고 rebase 전후 코드 diff 0), PR HEAD `967c9c8`, merge 가능
- Pi 재시험 3 을 `967c9c8` 에서 새 세션으로 시작했다, Opus 5.5 high 에게 6~8단계 검토를 맡겼다
