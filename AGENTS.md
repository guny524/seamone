# 1. 구조와 검사

- host의 native 작업은 `seamone_runtime::LibraryService`를 호출한다, 직접 SQL과 음성 저장 경로를 더하면 재생 증거와 claim 검사를 건너뛸 수 있다, `make check-architecture`가 Cargo 의존 방향과 adapter 주인을 검사한다
- runtime의 Store, AudioLibrary, Connection과 raw mutate는 공개하지 않는다, host가 Store를 직접 바꾸는 길은 `make test`의 통과/compile_fail doctest와 `make check-architecture`의 `scripts/runtime_api.json` 대조로 막는다, 공개 항목을 목록에 더하기 전에 LibraryService의 증거 확인 경로를 거치는지 검토한다
- Store의 검수 저장은 비공개 playback_evidence 모듈의 AuthorizedReview만 받는다, 새 확인 전사가 생기거나 바뀌면 admit이 Played 승인을 요구한다, issue_playback_token·mark_playback_ended·authorize_review 호출은 LibraryService의 playback·playback_ended·review와 cfg(test)에 고정하고 함수 범위와 base 목록으로 대조한다, 실제 청취나 최소 재생 시간 정책은 바꾸지 않는다
- token 발급 뒤 끝 알림을 바로 보내는 길은 host와 runtime 안 호출자 모두에게 남는다, 승인은 완료 알림과 token 범위를 확인하며 실제 청취나 최소 재생 시간은 확인하지 않는다, 이 검수 정책을 바꾸려면 사람 결정이 필요하다
- Rust unsafe는 scripts/unsafe_files.json에 적은 기존 마이크 권한 함수의 이름과 본문 해시가 같을 때만 쓴다, capture의 저장소·재생 증거·transmute 접근과 runtime include!는 거절한다, make check-unsafe와 base 대조가 이 범위를 확인한다
- VerifiedText의 필드는 core의 검수 전이 밖에 공개하지 않고 읽기는 getter로 한다, DB와 IPC 읽기를 위한 Deserialize는 유지하므로 이 타입만으로 저장 권한을 보장하지 않는다, struct literal 거절/정상 doctest와 JSON 시험이 필드 경계와 직렬화 모양을 확인한다
- segments body 쓰기와 training_queue INSERT는 비공개 WritePermit의 고정 메서드 안에서 검사·직렬화·저장한다, 열린 permit은 밖으로 주지 않는다, 검수 저장은 DB의 현재 값과 승인된 core 전이를 대조한다, queue 대상·revision 열 UPDATE와 trigger를 통한 보호 쓰기도 거절한다, 저장 전용 module의 타입·권한·구현 해시를 base와 대조하고 cfg(test) module의 시험 본문은 뺀다
- runtime_api.json, playback_api.json, review_writes.json, unsafe_files.json은 LINT_BASE_REF의 같은 목록과 대조한다, base에 목록이 없거나 항목을 스스로 늘리면 실패한다, 사람이 확인한 뒤에만 늘리며 이 PR의 CI는 사람이 보고 판단한다
- 학습 입력은 현재 원본과 확인된 전사 버전이 일치하는 EligibleExample만 쓴다, `LocalTrainer`의 doctest와 runtime 행동 시험이 일반 전사 입력, 오래된 재생 증거와 중복 claim을 거절한다
- 도메인은 OS I/O에 의존하지 않는다, `make check-domain`의 no_std 컴파일과 `make check-architecture`의 Cargo 검사가 이 경계를 지킨다
- IPC 이름은 core의 catalog에 한 번만 쓴다, 같은 catalog로 handler와 build 권한 이름을 만들고 `make test`에서 창과 플랫폼 capability를 대조한다
- host와 runtime의 녹음 지원 플랫폼은 같아야 한다, host의 const assert가 컴파일할 때 CAPTURE_SUPPORTED와 host cfg를 대조한다, Whisper feature 전달은 `make test CARGO_FEATURES=desktop-whisper`에서 host를 통해서만 켜서 확인한다
- runtime의 `test-support`는 host의 dev 의존에서만 켠다, Cargo 경계 검사가 제품 의존이나 feature의 전달로 켜는 것을 막고 IPC 시험은 합성 녹음과 전사 fixture를 쓴다
- lint 예외는 없다, `make lint`와 `make check-architecture`는 비어 있는 `scripts/lint_exceptions.json`과 기준 ref(`LINT_BASE_REF`, 기본 `origin/develop`)를 대조해 Rust allow/expect와 Python noqa/type ignore, Cargo/Ruff 억제 추가를 거절한다, 공개 함수의 오류와 panic은 문서로 밝히고 값의 범위와 플랫폼별 책임은 타입과 함수로 나타낸다, lint 설정은 문서와 이름 lint를 켜거나 Ruff ignore를 줄이는 변경만 허용하고 나머지 표와 clippy 필수 명령은 유지한다, Ruff의 lint 밖 표 전체와 Ruff 필수 명령도 기준과 대조하며 Ruff extend는 거절한다, 기준 ref나 JSON을 읽지 못하면 실패한다
- Rust 일반 주석과 Python 주석은 바꿀 수 없는 외부 API, 플랫폼과 형식의 사정만 남긴다, 문서 주석과 docstring, shebang과 encoding 선언은 제외하며 docstring 이외의 Python 문자열 문장과 bytes 문장, f-string은 주석처럼 거절한다, `make check-comments`가 남은 주석의 경로와 원문, 본문 hash를 `scripts/comments.json` 및 기준 ref와 대조한다, 새 주석과 목록 추가는 거절하며 오류는 이름과 구조로 드러내거나 검사로 옮기라고 안내한다, `make lint`와 `make check-architecture`도 같은 검사를 실행한다
- test-support와 Whisper 없이도 runtime 제품 라이브러리에 죽은 코드가 없어야 한다, `make lint-runtime-minimal`을 `make lint`와 Linux, macOS CI에서 실행한다
- 목록 대조가 실패해도 컴파일 진단을 확인한다, lint 진입점은 정책 검사와 컴파일 target을 `make -k`로 각각 실행하며 minimal, native workspace, WASM 컴파일도 `make -k`로 나눠 실행하고 CI는 정책과 native, iOS, WASM, domain 컴파일을 다른 step으로 실행한다, 컴파일 step의 `!cancelled()`와 `make test-architecture`의 실행 시험이 정책 실패 뒤에도 Clippy를 실행하는지 확인한다
- research 삭제는 cache.remove_inside, .part 파일 공개는 cache.publish_partial 한 입구로 모은다, HF cache 삭제, rename과 위치 인자 하나인 replace, Make recipe의 rm과 eval/exec/compile을 검사한다, 문자열 replace는 허용한다, subprocess와 os 실행 허용 범위는 README의 미결정 정책을 따른다, 시험 코드에 직접 삭제 예외는 없다
- CI의 기준 선택은 guard-base reusable workflow 한 벌로 한다, PR base와 develop push의 첫 parent를 비교 기준으로 쓰며 develop의 정책은 승인된 HEAD에서 읽는다, 다른 브랜치는 develop merge-base를 쓰고 변경 없는 push의 자기 대조도 허용한다, tag push는 제외한다, base 검사에는 PR merge ref를 넣고 보호 파일 차이는 merge-base부터 head까지 본다, 필수 check는 별도 pull_request_target workflow의 guard-rules-review-trusted이며 head 코드를 실행하지 않는다

# 출처

- [pstack correct](https://github.com/cursor/plugins/tree/main/pstack/skills/correct): 문서보다 구조와 타입, 검사로 반복 실수를 막고 실제 실패와 정상 대조로 검사를 확인하는 기준
