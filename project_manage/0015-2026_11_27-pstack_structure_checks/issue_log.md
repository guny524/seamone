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
