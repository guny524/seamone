# 1. 구조와 검사

- host의 native 작업은 `seamone_runtime::LibraryService`를 호출한다, 직접 SQL과 음성 저장 경로를 더하면 재생 증거와 claim 검사를 건너뛸 수 있다, `make check-architecture`가 Cargo 의존 방향과 adapter 주인을 검사한다
- runtime의 Store, AudioLibrary, Connection과 raw mutate는 공개하지 않는다, host에서 증거 없이 검수 결과를 바꾸는 길을 막기 위해 `make test`의 통과/compile_fail doctest가 공개 범위를 확인한다
- 학습 입력은 현재 원본과 확인된 전사 버전이 일치하는 EligibleExample만 쓴다, `LocalTrainer`의 doctest와 runtime 행동 시험이 일반 전사 입력, 오래된 재생 증거와 중복 claim을 거절한다
- 도메인은 OS I/O에 의존하지 않는다, `make check-domain`의 no_std 컴파일과 `make check-architecture`의 Cargo 검사가 이 경계를 지킨다
- IPC 이름은 core의 catalog에 한 번만 쓴다, 같은 catalog로 handler와 build 권한 이름을 만들고 `make test`에서 창과 플랫폼 capability를 대조한다
- host와 runtime의 녹음 지원 플랫폼은 같아야 한다, `make test`가 CAPTURE_SUPPORTED를 host cfg와 대조한다, Whisper feature 전달은 `make test CARGO_FEATURES=desktop-whisper`에서 host를 통해서만 켜서 확인한다
- 새 lint 억제를 달지 않는다, `make lint`와 `make check-architecture`가 기존 억제 목록보다 늘어난 속성, Python 주석, Cargo/Ruff 설정을 거절한다, 기존 억제의 사람 승인과 만료일은 미결정이다

# 출처

- [pstack correct](https://github.com/cursor/plugins/tree/main/pstack/skills/correct): 문서보다 구조와 타입, 검사로 반복 실수를 막고 실제 실패와 정상 대조로 검사를 확인하는 기준
