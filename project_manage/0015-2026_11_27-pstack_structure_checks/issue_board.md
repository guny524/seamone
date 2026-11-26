# LLM 실수를 lint, build, compile 에서 거르게 폴더 구조와 검사를 바꾼다
(이 파일: 기준 브랜치 develop 의 이슈 board, 트래커 https://github.com/guny524/seamone/issues/15)
- 마지막 갱신: 2026-11-27 01:05 KST
- 다른 세션이 남긴 전달: 없음

## 1. 사용자 지시 원문
- 2026-11-27 00:5x "/overnight-autopilot worker 는 sol high 1개 두고 검토자는 opus 5.5 high 1개 두고 1시간 단위 cron 해서 아침 7시까지 돌아줘, 우리 lauren tan 의 pstack 을 레포에 본격적으로 적용해서 구조적으로 llm 이 실수를 하지 않게 lint 나 build compile 단에서 걸러낼 수 있게 pstack skill 의 개념들을 확인해서 최대한 레포에 적용하는 작업을 밤 새 할거야 아마 우선 폴더 구조를 개선하는데 많은 시간을 써야 할거야"
- 2026-11-27 같은 메시지 "local 에 pstack을 설치하지는 말고 github 에서 참고해서 내용만 확인해서 프로젝트에 적용을 해 여기 local 노트북에 pstack 을 깔면 안돼, 아니면 다른 폴더 임시로 만들어두고 거기서만 적용한 다음에 worker 를 거기서 소한해서 사용하던가 해도 돼"
- 2026-11-27 같은 메시지 "pstack 의 이념이 레포에 잘 적용되었는지 확인을 하려면 우리 pi 로 local llm 을 사용 할 수 있거든? d0xin-swift-1.5-qwen3.8-27b-uncensored-fp8 걔를 worker 로 써서 뭔가를 시켜보고 어떻게 하는지 모니터링을해서 (걔한테 작업 시키기 전에 git status git diff 로 내용 확인해두고, 작업 끝나면 git diff 로 확인해보는 식으로도 모니터링 할 수 있고, orca cli 로 pi 터미널 창에서 출력하는 내용을 볼 수도 있고 /conversation-rag skill 로 그 agent 가 했던 내역을 볼 수도 있어) 확인하는 방법으로 제대로 잘 적용이 됐다는 걸 검증할 수 있어 - https://github.com/cursor/plugins/tree/main/pstack"
- 2026-11-27 01:0x "그래, 이 프로젝트 안에서만 적용되게 하는 건 괜찮아 ~/llms 나 user space 건들지 말라는 소리야"
- 2026-11-27 01:0x 질문 답: 열린 PR 은 "worker 가 새 구조로 옮김", lint 예외는 "밤에는 새 예외 금지"

## 2. 배경
- 2026-11-25 codex 세션이 pstack 의 `correct` skill 과 영상(https://www.youtube.com/watch?v=nldpxmin-1M)을 참고해 crate 의존 경계, IPC 명령 목록, 검수 동작 조건, lint 예외 검사를 넣었다(PR #12, `878e9c5`, 프로젝트 log 2026-11-25 18:20)
- 이번 이슈는 그 위에 폴더 구조와 검사를 더해, 한 파일만 본 agent 가 가장 짧은 길로 고쳐도 레포 전체에 맞는 변경만 컴파일과 lint 를 통과하게 한다

## 3. 목표와 범위
### 3-1. 요구
- 실수 부류를 실제 기록(git log, revert, 검토 기록, 우회 주석)에서 찾고, 두 번 이상 나온 것만 부류로 센다
- 부류마다 가장 높은 단계에서 막는다: 구조(상태마다 주인 하나, 일마다 길 하나, 내부를 숨겨 잘못된 import 가 실패) -> 타입(잘못된 상태를 쓸 수 없게) -> 고칠 파일과 타입과 함수를 오류 글에 적는 lint 와 CI 검사 -> 행동 시험 -> 문서와 agent 규칙은 판단이 필요한 것만 마지막에
- 새 검사마다 실제 과거 실수 하나를 되살려 실패하는 것을 증명한다, 로컬과 CI 가 같은 make target 을 쓴다
- 예외는 그 줄에 이유, 만료일, 사람 승인을 다는 형식만 허용하고 형식을 검사로 강제한다, 밤에는 새 예외를 달지 않는다(사용자 답)
- 레포 안 `AGENTS.md` 에 규칙과 그 규칙을 강제하는 검사의 짝 목록을 둔다
- 폴더 구조 개선에 가장 큰 비중을 둔다
### 3-2. 범위
- seamone 레포 안의 코드, 폴더, Makefile, CI, README, `AGENTS.md`, `scripts/`
- 열린 초안 PR #11(첫 전사), #13, #14(연구 코드)를 새 구조로 rebase 한다(사용자 답)
### 3-3. 제외 범위
- 레포 밖(사용자 홈의 하네스 설정 `~/llms`, `~/.claude`, `~/.codex`, `~/.pi`, 다른 레포)은 읽기만 한다
- pstack 을 노트북에 설치하거나 파일로 받지 않는다, GitHub 에서 화면 출력으로만 읽는다
- 노트북에 앱을 설치하거나 실행하지 않는다(프로젝트 board 3절의 2026-11-25 지시)
- 동작의 뜻(합의한 계약, PD1~PD15)을 바꾸지 않는다
### 3-4. 완료 기준
- `make lint`, `make test`, `make check-architecture` 가 develop 과 CI 에서 통과한다
- 고른 부류마다 되살린 과거 실수가 검사에서 실패하는 기록이 이 이슈 log 에 있다
- local LLM worker 의 실수 유도 시험에서 검사가 실수를 거른 기록과 못 거른 것의 처리가 이 이슈 log 에 있다
### 3-5. 검증 방법
- 메인이 diff 와 make target 을 직접 돌린다, CI 결과를 확인한다, Opus 5.5 high 검토자가 변경과 레포만 보고 검토한다
### 3-6. 의존성과 관련 링크
- pstack: https://github.com/cursor/plugins/tree/main/pstack (`skills/correct`, `principle-encode-lessons-in-structure`, `principle-boundary-discipline`, `principle-type-system-discipline`, `principle-model-the-domain`, `architect`)
- 앞 작업: PR #12 https://github.com/guny524/seamone/pull/12

## 7. 할 일과 검증
- [ ] 7-1. 실수 부류 조사와 근거
- [ ] 7-2. 폴더 구조 두 안 비교와 고르기, 검토자 검토
- [ ] 7-3. 폴더 구조 변경과 Makefile, CI, README 맞추기
- [ ] 7-4. 부류별 검사 추가와 과거 실수 되살리기 증명
- [ ] 7-5. 예외 형식 검사와 `AGENTS.md` 규칙과 검사 짝 목록
- [ ] 7-6. PR, CI, 검토, develop merge
- [ ] 7-7. 열린 초안 PR #11, #13, #14 rebase
- [ ] 7-8. local LLM 실수 유도 시험과 빠진 곳 메우기

## 9. 지금 상태
- 2026-11-27 01:05 이슈와 기록을 만들었다, worker 를 띄운다
