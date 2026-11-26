# 1. 구조와 검사

- host의 native 작업은 `seamone_runtime::LibraryService`를 호출한다, 직접 SQL과 음성 저장 경로를 더하면 재생 증거와 claim 검사를 건너뛸 수 있다, `make check-architecture`가 Cargo 의존 방향과 adapter 주인을 검사한다
- runtime의 Store, AudioLibrary, Connection과 raw mutate는 공개하지 않는다, host가 Store를 직접 바꾸는 길은 `make test`의 통과/compile_fail doctest와 `make check-architecture`의 `scripts/runtime_api.json` 대조로 막는다, 공개 항목을 목록에 더하기 전에 LibraryService의 증거 확인 경로를 거치는지 검토한다
- Store의 검수 저장은 비공개 playback_evidence 모듈이 발급한 AuthorizedReview만 받는다, PlaybackProof와 승인 값의 생성자 및 필드는 그 모듈 밖에서 열지 않는다, core의 required_evidence가 재생을 요구하는 요청은 같은 crate 안에서도 재생 완료와 token 범위를 확인한 뒤에만 저장하도록 하며 `make test`가 실제 모듈 소스를 쓰는 같은 crate의 정상/compile_fail 짝으로 확인한다
- host는 재생을 듣지 않고 `playback_ended`를 바로 부를 수 있다, 최소 재생 시간 확인은 현재 검수 정책에 없으며 바꾸려면 사람 결정이 필요하다
- 학습 입력은 현재 원본과 확인된 전사 버전이 일치하는 EligibleExample만 쓴다, `LocalTrainer`의 doctest와 runtime 행동 시험이 일반 전사 입력, 오래된 재생 증거와 중복 claim을 거절한다
- 도메인은 OS I/O에 의존하지 않는다, `make check-domain`의 no_std 컴파일과 `make check-architecture`의 Cargo 검사가 이 경계를 지킨다
- IPC 이름은 core의 catalog에 한 번만 쓴다, 같은 catalog로 handler와 build 권한 이름을 만들고 `make test`에서 창과 플랫폼 capability를 대조한다
- host와 runtime의 녹음 지원 플랫폼은 같아야 한다, host의 const assert가 컴파일할 때 CAPTURE_SUPPORTED와 host cfg를 대조한다, Whisper feature 전달은 `make test CARGO_FEATURES=desktop-whisper`에서 host를 통해서만 켜서 확인한다
- runtime의 `test-support`는 host의 dev 의존에서만 켠다, Cargo 경계 검사가 제품 의존이나 feature의 전달로 켜는 것을 막고 IPC 시험은 합성 녹음과 전사 fixture를 쓴다
- 새 lint 억제를 달지 않는다, `make lint`와 `make check-architecture`는 `scripts/lint_exceptions.json`을 기준 ref(`LINT_BASE_REF`, 기본 `origin/develop`)와 비교해 억제 추가와 개수 증가를 막고 Cargo/Ruff lint 표 전체와 clippy 명령도 대조한다, 기준 ref나 JSON을 읽지 못하면 실패한다, 기존 억제의 사람 승인과 만료일은 미결정이다

# 출처

- [pstack correct](https://github.com/cursor/plugins/tree/main/pstack/skills/correct): 문서보다 구조와 타입, 검사로 반복 실수를 막고 실제 실패와 정상 대조로 검사를 확인하는 기준
