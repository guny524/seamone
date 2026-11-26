# 1. 구조와 검사

- host의 native 작업은 `seamone_runtime::LibraryService`를 호출한다, 직접 SQL과 음성 저장 경로를 더하면 재생 증거와 claim 검사를 건너뛸 수 있다, `make check-architecture`가 Cargo 의존 방향과 adapter 주인을 검사한다
- runtime의 Store, AudioLibrary, Connection과 raw mutate는 공개하지 않는다, host가 Store를 직접 바꾸는 길은 `make test`의 통과/compile_fail doctest와 `make check-architecture`의 `scripts/runtime_api.json` 대조로 막는다, 공개 항목을 목록에 더하기 전에 LibraryService의 증거 확인 경로를 거치는지 검토한다
- Store의 검수 저장은 비공개 playback_evidence 모듈의 AuthorizedReview만 받는다, 재생이 필요한 요청의 token은 Playbacks::issue가 현재 조각 조회와 revision 대조, 해당 구간 WAV 준비를 마친 뒤에만 발급한다, begin과 증거 필드는 모듈 비공개이며 Default 생성 경로는 두지 않는다, make test의 정상/compile_fail 짝과 make check-architecture의 scripts/playback_api.json 대조가 이 경계를 확인한다
- token 발급 뒤 끝 알림을 바로 보내는 길은 host와 runtime 안 호출자 모두에게 남는다, 승인은 완료 알림과 token 범위를 확인하며 실제 청취나 최소 재생 시간은 확인하지 않는다, 이 검수 정책을 바꾸려면 사람 결정이 필요하다
- Rust unsafe는 기존 native adapter인 crates/seamone-runtime/src/capture.rs에서만 쓴다, make check-unsafe가 scripts/unsafe_files.json과 기준 ref(LINT_BASE_REF)의 허용 파일을 대조해 다른 파일의 unsafe와 허용 파일 추가를 거절한다, make check-architecture와 lint도 이 검사를 실행한다
- VerifiedText의 필드는 core의 검수 전이 밖에 공개하지 않고 읽기는 getter로 한다, DB와 IPC 읽기를 위한 Deserialize는 유지하므로 이 타입만으로 저장 권한을 보장하지 않는다, struct literal 거절/정상 doctest와 JSON 시험이 필드 경계와 직렬화 모양을 확인한다
- 확인 전사가 담길 수 있는 segments body 쓰기와 training_queue INSERT는 Store::mutate(AuthorizedReview)로 모은다, make check-review-writes가 Rust 함수 범위와 SQLite의 SQL 해석으로 다른 위치의 쓰기와 동적 SQL을 거절한다, 기존 미검증 전사 생성과 검증 해제 함수는 검토한 본문 해시로만 예외를 허용한다
- runtime_api.json, playback_api.json, review_writes.json, unsafe_files.json은 LINT_BASE_REF의 같은 목록과 대조한다, base에 목록이 없거나 항목을 스스로 늘리면 실패한다, 사람이 확인한 뒤에만 늘리며 이 PR의 CI는 사람이 보고 판단한다
- 학습 입력은 현재 원본과 확인된 전사 버전이 일치하는 EligibleExample만 쓴다, `LocalTrainer`의 doctest와 runtime 행동 시험이 일반 전사 입력, 오래된 재생 증거와 중복 claim을 거절한다
- 도메인은 OS I/O에 의존하지 않는다, `make check-domain`의 no_std 컴파일과 `make check-architecture`의 Cargo 검사가 이 경계를 지킨다
- IPC 이름은 core의 catalog에 한 번만 쓴다, 같은 catalog로 handler와 build 권한 이름을 만들고 `make test`에서 창과 플랫폼 capability를 대조한다
- host와 runtime의 녹음 지원 플랫폼은 같아야 한다, host의 const assert가 컴파일할 때 CAPTURE_SUPPORTED와 host cfg를 대조한다, Whisper feature 전달은 `make test CARGO_FEATURES=desktop-whisper`에서 host를 통해서만 켜서 확인한다
- runtime의 `test-support`는 host의 dev 의존에서만 켠다, Cargo 경계 검사가 제품 의존이나 feature의 전달로 켜는 것을 막고 IPC 시험은 합성 녹음과 전사 fixture를 쓴다
- 새 lint 억제를 달지 않는다, `make lint`와 `make check-architecture`는 `scripts/lint_exceptions.json`을 기준 ref(`LINT_BASE_REF`, 기본 `origin/develop`)와 비교해 억제 추가와 개수 증가를 막고 Cargo/Ruff lint 표 전체와 clippy 명령도 대조한다, 기준 ref나 JSON을 읽지 못하면 실패한다, 기존 억제의 사람 승인과 만료일은 미결정이다
- research의 파일 삭제 API는 `seamone_research.cache.remove_inside` 한 입구에서만 쓴다, 전용 eval-raw/seamone_eval cache의 경계를 확인하고 공유 HF_HOME/hub를 보호하기 위해 `make check-architecture`와 research lint의 AST 검사 및 경로 행동 시험을 함께 쓴다, research 시험 코드도 직접 삭제를 예외로 허용하지 않는다

# 출처

- [pstack correct](https://github.com/cursor/plugins/tree/main/pstack/skills/correct): 문서보다 구조와 타입, 검사로 반복 실수를 막고 실제 실패와 정상 대조로 검사를 확인하는 기준
