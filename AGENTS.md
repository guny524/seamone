# 1. 구조와 검사

- host의 native 작업은 `seamone_runtime::LibraryService`를 호출한다, 직접 SQL과 음성 저장 경로를 더하면 재생 증거와 claim 검사를 건너뛸 수 있다, `make check-architecture`가 Cargo 의존 방향과 adapter 주인을 검사한다
- runtime의 Store, AudioLibrary, Connection과 raw mutate는 공개하지 않는다, host가 Store를 직접 바꾸는 길은 `make test`의 통과/compile_fail doctest와 `make check-architecture`의 `scripts/runtime_api.json` 대조로 막는다, 공개 항목을 목록에 더하기 전에 LibraryService의 증거 확인 경로를 거치는지 검토한다
- Store의 검수 저장은 비공개 playback_evidence 모듈의 AuthorizedReview만 받는다, 새 확인 전사가 생기거나 바뀌면 admit이 Played 승인을 요구한다, token 발급·완료·승인 호출은 LibraryService의 playback·playback_ended·review와 cfg(test)에 고정하고 함수 범위와 base 목록으로 대조한다, 실제 청취나 최소 재생 시간 정책은 바꾸지 않는다
- token 발급 뒤 끝 알림을 바로 보내는 길은 host와 runtime 안 호출자 모두에게 남는다, 승인은 완료 알림과 token 범위를 확인하며 실제 청취나 최소 재생 시간은 확인하지 않는다, 이 검수 정책을 바꾸려면 사람 결정이 필요하다
- Rust unsafe는 scripts/unsafe_files.json에 적은 기존 마이크 권한 함수의 이름과 본문 해시가 같을 때만 쓴다, capture의 저장소·재생 증거·transmute 접근과 runtime include!는 거절한다, make check-unsafe와 base 대조가 이 범위를 확인한다
- VerifiedText의 필드는 core의 검수 전이 밖에 공개하지 않고 읽기는 getter로 한다, DB와 IPC 읽기를 위한 Deserialize는 유지하므로 이 타입만으로 저장 권한을 보장하지 않는다, struct literal 거절/정상 doctest와 JSON 시험이 필드 경계와 직렬화 모양을 확인한다
- 확인 전사가 담길 수 있는 segments body 쓰기와 training_queue INSERT는 SQLite authorizer의 검수 권한이 열린 구간에서만 한다, 초기 미검증 전사와 검증 해제에는 확인 전사 없음 검사 뒤 별도 권한을 주고 본문 해시를 고정한다, trigger/view 생성과 trigger를 통한 보호 쓰기, 권한이 닫힌 뒤의 prepared statement도 거절한다, 정적 검사와 실제 SQLite 시험이 이 경계를 확인한다
- runtime_api.json, playback_api.json, review_writes.json, unsafe_files.json은 LINT_BASE_REF의 같은 목록과 대조한다, base에 목록이 없거나 항목을 스스로 늘리면 실패한다, 사람이 확인한 뒤에만 늘리며 이 PR의 CI는 사람이 보고 판단한다
- 학습 입력은 현재 원본과 확인된 전사 버전이 일치하는 EligibleExample만 쓴다, `LocalTrainer`의 doctest와 runtime 행동 시험이 일반 전사 입력, 오래된 재생 증거와 중복 claim을 거절한다
- 도메인은 OS I/O에 의존하지 않는다, `make check-domain`의 no_std 컴파일과 `make check-architecture`의 Cargo 검사가 이 경계를 지킨다
- IPC 이름은 core의 catalog에 한 번만 쓴다, 같은 catalog로 handler와 build 권한 이름을 만들고 `make test`에서 창과 플랫폼 capability를 대조한다
- host와 runtime의 녹음 지원 플랫폼은 같아야 한다, host의 const assert가 컴파일할 때 CAPTURE_SUPPORTED와 host cfg를 대조한다, Whisper feature 전달은 `make test CARGO_FEATURES=desktop-whisper`에서 host를 통해서만 켜서 확인한다
- runtime의 `test-support`는 host의 dev 의존에서만 켠다, Cargo 경계 검사가 제품 의존이나 feature의 전달로 켜는 것을 막고 IPC 시험은 합성 녹음과 전사 fixture를 쓴다
- 새 lint 억제를 달지 않는다, `make lint`와 `make check-architecture`는 `scripts/lint_exceptions.json`을 기준 ref(`LINT_BASE_REF`, 기본 `origin/develop`)와 비교해 억제 추가와 개수 증가를 막고 Cargo/Ruff lint 표 전체와 clippy 명령도 대조한다, 기준 ref나 JSON을 읽지 못하면 실패한다, 기존 억제의 사람 승인과 만료일은 미결정이다
- research 삭제는 cache.remove_inside, .part 파일 공개는 cache.publish_partial 한 입구로 모은다, subprocess와 os의 실행 명령, HF cache 삭제, 직접 rename/replace와 Make recipe의 rm을 검사한다, 시험 코드에 직접 삭제 예외는 없다
- CI는 PR base 또는 push before의 검사 코드와 목록으로 head 소스를 별도 검사하고 보호 파일 변경은 사람이 확인할 때까지 실패시킨다, 새 브랜치는 develop과의 merge-base를 쓰며 base가 HEAD와 같으면 실패한다, pull_request_target 경로는 head 실행 없이 보호 파일 변경만 확인한다

# 출처

- [pstack correct](https://github.com/cursor/plugins/tree/main/pstack/skills/correct): 문서보다 구조와 타입, 검사로 반복 실수를 막고 실제 실패와 정상 대조로 검사를 확인하는 기준
