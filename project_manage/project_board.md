# seamone 프로젝트 board
(이 파일: 기준 브랜치 main 의 `project_manage/project_board.md`, 같은 곳에 `project_decision.md`, `project_log.md`, 자리와 이름은 rule `project_convention.md`, 형식은 `project-issue-conductor` skill 의 `references/record-format.md`)
- 마지막 갱신: 2026-11-19 08:12
- 다른 세션이 남긴 전달: 없음

## 1. 프로젝트 맥락
- seamone 은 사용자 개인 프로젝트다, 기기 안에서 음성을 받아쓰고 음성과 글을 밖으로 보내지 않은 채 FL 로 받아쓰기 모델을 키우는 앱을 만든다
- 출발 메모(사용자 개인 노트 "local stt + fl", 2026-09-21): "내 목소리 다른 사람 목소리 녹음 한거 local 에서 stt 할 수 있는 앱 / 사람이 결과 보고 수동으로 고쳐서 라벨링할 수 있다 / fl 로 연결해서 데이터는 안보내고 모델 업데이트만 한다"
- 사용자가 ChatGPT 에 물은 글(2026-11-19 사용자가 붙여 줌): "나는 fl 로 학습시키기 가장 좋은 소재가 stt 인거 같거든 특히 한국어 stt? 근데 여러 다자 회의 내용 그대로 전사 하는 거랑 요약 하는게 제일 쓸모가 많다고 생각하고 이거 그냥 데스크탑 앱 핸드폰 앱으로 빨리 만들어서 배포하고 사람들이 많이 쓰면 쓸수록 fl이니까 성능이 올라갈거잖아 근데 문제가 이거 정답 라벨링을 어떻게 할까? pretrain 다음 단어 맞추는 거처럼 어떻게 라벨링 없이 stt 성능을 높힐지 아이디어를 내봐 아니면 gan 처럼 뭔가 적대적으로 stt 를 시킬 수 있나?"
- 레포: https://github.com/guny524/seamone (공개, Apache-2.0), 트래커는 GitHub 이슈, 기준 브랜치 main
- 큰 그림의 결정은 `project_decision.md` PD1~PD12

## 2. 사용자 지시 원문
- 2026-11-19 "나는 이거 맥이든 윈도우든 리눅스든 다 돌아가고 핸드폰에서도 됐으면 좋겠어. 그리고 모델은 굳이 오픈소스 모델을 쓸 필요 없지. 우리가 어차피 파인튜닝을 할 거니까 FL로. 근데 이제 처음에 시작할 모델을 뭘로 할지만 정하는 게 좀 필요해. 근데 영어나 뭐 최대한 다양한 언어나 됐으면 좋겠고, 특히 한국어가 잘 돼야 돼."
- 2026-11-19 "그리고 이게 핸드폰에서도 다 됐으면 좋겠고, 상관없이 다 됐으면 좋겠어. 그리고 최대한 고나 로스트 같은 걸로 그냥 빌드가 됐으면 좋겠고,"
- 2026-11-19 "근데 이제 결과물이 뭐 맥에 DSG, DMSG 뭐 하여튼 그 설치 파일 뭐 그걸로 들어가고, 윈도우는 뭐 EXE 파일 설치 파일로 뭐 돼야 될 테고, 리눅스는 뭐 대 파일 뭐 그런 걸로 돼야겠지. 근데 그 핸드폰은 안드로이드 APK나 뭐 iOS 걸로 돼야 되니까 멀티 플랫폼 될 수 있는 걸로 하는 게 나을 것 같아. 그리고 만약에 그런 프레임워크 쓰는 게 러스트나 고가 없어도 그래도 강타입 언어로 이제 컴파일 되는 애들이었으면 좋겠어. 러스트면 베스트고."
- 2026-11-19 "그리고 이제 아까 최종적으로 보고 있는 모델 크기는 CPU에서 돌아가는 모델 크기였으면 좋겠어. 한 3빌리언 정도면 그래도 CPU에서 빨리 돌지 않을까? 이게 만약에 그런 모델이 없다고 하더라도 우리가 그냥 토치로 모델을 잘 설계해서 기존 다른 STT 모델로부터 디스틸레이션 해가지고 그냥 해도 괜찮을 것 같아 중요한 게 로컬에서 다른 사람들이 그 녹음을 하잖아. 그거 가지고 어떻게 성능을 높일, 라벨링 없이도 언슈퍼바이즈드로 어떻게 성능을 높일 수 있는지 그런 걸 좀 강구하는 게 필요해."
- 2026-11-19 "sea anemone 라서 seamone 라고 하자"
- 2026-11-19 "30분 간격으로 크론 돌고 아침 9시에 끝내. 워커는 소넷 맥스 2개, 검토자는 솔 하이, 이거 깃허브에 레포도 만들었고 지금 현재 네 pwd 처가 네 위치도 시몬의 git clone 받은 위치니까 이제 너가 알아서 자율주행 해야 하거든."
- 2026-11-19 "나 잘 거니까 스톡 걸릴 것 같은 거 물어보라고"
- 2026-11-19 "GitHub 액션에서 CI로만 빌드를 해. 폰에서 폰 AP 가속기 써가지고 못해? 최대한 일단 할 수 있는 거에서 모델 크기가 컸으면 좋겠어. 아니면은 폰과 노트북 모델은 따로 가는 걸로 하자. 그 노트북이나 그런 데서도 맥 같은 거는 GPU 그 AP 가속기를 쓸 수가 있잖아. M1 이런 것들은 그런 거 좀 감안을 하고 우리 CPU에서 계산할 때도 심드 같은 걸로 최대한 가속해서 계산할 수 있게끔 하고 아 됐어, 시몬 그냥 해. 이름 뭐 나중에 바꾸든가 해. 무료로 가져다 쓸 수 있고 할 수 있는 라이센스로 그냥 그래, 프로젝트 매니지의 포지션만 보여도 돼. 이 프로젝트는 프로젝트 manage git 이그노에다가 넣어놓고 하든가. 아니야 폰에 있는 것도 학습을 해야 해. 노트북에 도구 설치는 하지마. 아이 뭐 카고, 타우리, 트렁크, 왓슨타기 이런건 괜찮아. 터치 뭐 되는데 그 볼륨 없으니까 그 우리 롤과 스킬에 쓰고 나면은 찌꺼기들 잘 지우라고 해놓은 거 있거든. 그거 잘 준수해서 용량 크게 안 남게 해. 그래, 엑스코드와 JDX는 안 설치하는 게 맞지. 근데 자바가 왜 필요해?"
- 2026-11-19 "어쨌든 모르겠고 이제 나한테 물어보지 말고 지금 너 /conversation-rag 보면은 예전에 너 스스로 모델 바꿔서 한 거 있거든. 그러니까 지금은 페이블 5.2로 플랜 세울 때 동안 페이블 5.2 엑스트라 하이로 가고, 그 이후부터는 소넷 맥스로 너가 스스로 바꿔서 계속 진행을 해."

## 3. 운영 방침
- 우선순위 문서: 이 board 5절
- 동시 worker 수와 모델: Sonnet max 2개(2026-11-19 밤샘 실행), 한도가 끝나면 Gemini 3.8 Flash high(기본값)
- 검토 모델: Codex Sol high(2026-11-19 사용자), 검토 지적은 그 코드를 쓴 worker 가 고친다(기본값), 자문 모델: 정하지 않음
- 메인 모델: 계획은 Fable 5.2 xhigh, 계획 뒤부터 Sonnet max(2026-11-19 사용자, 메인이 스스로 바꾼다), 밤이 끝나면 바꾸기 전 설정으로 되돌린다
- 위임: 2026-11-19 "어쨌든 모르겠고 이제 나한테 물어보지 말고 ... 계속 진행을 해", 범위는 이 레포에 push, GitHub 이슈와 PR 만들기와 merge, GitHub Actions 실행, PD4 가 허용한 도구 설치, 유보한 단계: 메일과 Slack 보내지 않음, 집 서버와 유료 클라우드를 쓰지 않음, 서명과 앱 장터 배포를 하지 않음, 레포 설정(공개 범위)을 바꾸지 않음, 끝나는 때: 2026-11-19 09:00
- 점검 간격: 30분(2026-11-19 사용자, 밤샘 실행 동안 그대로 쓴다)
- 멈추는 시각: 2026-11-19 09:00, 일이 다 끝나면 그 전에 cron 을 지운다
- 채우기 중단: 없음
- 환경과 자원 제약: 노트북 M1 Air 16GB, 디스크 여유 49GB(2026-11-19 03:50), 무거운 일(모델 학습과 큰 cargo 빌드)은 한 번에 하나씩, 내려받는 것은 `/Users/min-jo/Documents/Developments/seamone/.tmp/hf` 한 곳에(PD12), 노트북은 전원을 꽂고 덮개를 열어 둔다
- 다른 프로젝트와 합의한 것: 없음

## 4. 이슈
### #1 앱 뼈대(Tauri 2 + Leptos)를 세우고 다섯 플랫폼 설치 파일을 CI 로 만든다 (빈 앱으로 다섯 플랫폼 묶기를 먼저 확인한다)
- 상태: 재검토 중(두 번째 Codex 검토, 08:07 시작), 고친 커밋 12bb603 의 CI 는 두 번 모두 통과했고 Android 키 고정을 확인했다(draft PR https://github.com/guny524/seamone/pull/6)
- 폴더: `project_manage/0001-2026_11_19-app_skeleton_ci` (절대 경로 `/Users/min-jo/Documents/Developments/seamone/project_manage/0001-2026_11_19-app_skeleton_ci`)
- 작업: 브랜치 `guny524/1-app-skeleton-ci`, worktree `/Users/min-jo/orca/workspaces/seamone/1-app-skeleton-ci`, worker app(Sonnet max), 터미널 `term_b522cdf5-deb1-4bf0-a0ca-285d2f4bc264`, 세션 `2e24423d-9a31-4d8f-8f08-04d4faaad311`, 이어하기 `claude --resume 2e24423d-9a31-4d8f-8f08-04d4faaad311 --model sonnet --effort max --permission-mode bypassPermissions`, 마지막 지시 2026-11-19 06:12(CI 로그를 읽고 고치기), 받은 보고 `project_manage/0001-2026_11_19-app_skeleton_ci/.tmp/report-1.md`, 다음 보고 `project_manage/0001-2026_11_19-app_skeleton_ci/.tmp/report-2.md`, 검토자 Codex Sol high 읽기 전용(Orca 터미널 `term_2f4338a7-15ec-4163-90c5-7a5141a627c8`, 세션 `01a1f08d-4843-7021-a551-da6656dd58f7`, 2026-11-19 06:25 시작, 결과 `.tmp/review/codex-1.md`, 끝남 표시 `.tmp/review/codex-1.err` 의 DONE)
- 선행: 없음, 후속: #3 받아쓰기 엔진 interface 와 속도 측정, #5 녹음과 받아쓰기와 수정 화면
- 막힌 이유: 없음
- 다음 행동: 재검토 결과(`.tmp/review/codex-3.md`)를 읽고 이전 지적 네 개가 고쳐졌는지와 새 지적을 원문으로 확인한다, 통과하면 커밋 직전 증거 대조 후 draft 를 풀고 merge 한다, 사람 시험(설치 파일 실행)이 남아 있어 이슈는 닫지 않는다, app worker 는 받아쓰기 엔진 이슈의 앞 절반을 같은 worktree 에서 하는 중

### #2 시작 모델 후보의 한국어 오류율을 같은 잣대로 잰다 (출발점 모델을 고를 근거의 절반)
- 상태: 검토 지적 반영 중(커밋 d8b92ee, draft PR https://github.com/guny524/seamone/pull/7, Codex Sol high 검토에서 여섯 개를 받아 research worker 가 08:30 까지 고치는 중)
- 폴더: `project_manage/0002-2026_11_19-start_model_cer` (절대 경로 `/Users/min-jo/Documents/Developments/seamone/project_manage/0002-2026_11_19-start_model_cer`)
- 작업: 브랜치 `guny524/2-start-model-cer`, worktree `/Users/min-jo/orca/workspaces/seamone/2-start-model-cer`, worker research(Sonnet max), 터미널 `term_f05b49b0-2a37-4baf-95d0-b68df26ce2b7`, 세션 `0ee282a2-407f-4aab-8af7-8d53d330a854`, 이어하기 `claude --resume 0ee282a2-407f-4aab-8af7-8d53d330a854 --model sonnet --effort max --permission-mode bypassPermissions`, 마지막 지시 2026-11-19 07:56(다음 일로 라벨 없는 FL 모의실험), 받은 보고 `project_manage/0002-2026_11_19-start_model_cer/.tmp/report-1.md`, 검토자 Codex Sol high 읽기 전용(Orca 터미널 `term_6321555b-e1cf-4596-a9e7-2f1acd2961d1`, 07:55 시작, 결과 `.tmp/review/codex-2.md`)
- 선행: 없음, 후속: #4 라벨 없는 FL 모의실험, PD9 시작 모델 결정
- 막힌 이유: 없음
- 다음 행동: research worker 가 review-cells-2.md 의 여섯 칸(7-9 ~ 7-14)을 고쳐 report-2.md 를 쓰면 메인이 확인해 커밋하고 push, 재검토, 맨 위 CI 가 research 를 돌게 하는 일(맨 위 `ci.yml` 은 앞 이슈가 merge 된 뒤에 더한다), merge, 이 이슈는 사람 시험이 없어 닫는다

### #3 받아쓰기 엔진 interface 를 세우고 기기별 속도를 잰다 (출발점 모델을 고를 근거의 나머지 절반)
- 상태: 진행(앞 절반만: interface 와 가짜 엔진과 엔진 후보 조사, 무거운 일 없이 08:35 까지)
- 폴더: `project_manage/0003-2026_11_19-asr_engine_speed`
- 작업: 앞 뼈대 이슈와 같은 worktree `/Users/min-jo/orca/workspaces/seamone/1-app-skeleton-ci`(새 파일은 메인이 경로로 골라 별도 브랜치 `guny524/3-asr-engine-speed` 로 커밋한다), worker app(Sonnet max), 터미널 `term_b522cdf5-deb1-4bf0-a0ca-285d2f4bc264`, 마지막 지시 2026-11-19 08:00, 보고 `project_manage/0003-2026_11_19-asr_engine_speed/.tmp/report-1.md`
- 선행: #1 앱 뼈대(재검토 중, 코드는 같은 worktree 에 있다), 후속: #5 녹음과 받아쓰기와 수정 화면, PD9 시작 모델 결정
- 막힌 이유: 속도 측정(뒤 절반)은 모델 내려받기와 C++ 빌드가 무거워 노트북이 한가할 때 한다
- 다음 행동: app worker 가 08:35 에 보고하면 메인이 확인하고 별도 브랜치로 push 한다, 속도 측정은 다음 밤샘이나 사용자가 정할 때

### #4 라벨 없는 FL 모의실험으로 오류율을 낮추는 방법을 고른다 (이 프로젝트의 핵심 물음)
- 상태: 진행(작은 규모로 줄여 08:35 까지, 파이프라인을 끝까지 돌려 보고 다음에 큰 규모로 돌릴 코드와 명령을 남긴다)
- 폴더: `project_manage/0004-2026_11_19-unlabeled_fl_sim`
- 작업: 시작 모델 오류율 이슈와 같은 worktree `/Users/min-jo/orca/workspaces/seamone/2-start-model-cer`(새 파일은 메인이 경로로 골라 커밋하고 별도 브랜치 `guny524/4-unlabeled-fl-sim` 으로 push 한다), worker research(Sonnet max), 터미널 `term_f05b49b0-2a37-4baf-95d0-b68df26ce2b7`, 세션 `0ee282a2-407f-4aab-8af7-8d53d330a854`, 이어하기 `claude --resume 0ee282a2-407f-4aab-8af7-8d53d330a854 --model sonnet --effort max --permission-mode bypassPermissions`, 마지막 지시 2026-11-19 07:56, 보고 `project_manage/0004-2026_11_19-unlabeled_fl_sim/.tmp/report-1.md`
- 선행: 시작 모델 오류율 이슈(검토 중, 평가 코드와 데이터를 쓴다), 후속: 뒤의 학습 엔진 이슈
- 막힌 이유: 없음(앞 이슈의 코드는 같은 worktree 에 있다)
- 다음 행동: research worker 가 작은 규모로 돌려 08:35 에 보고한다, 메인이 확인하고 별도 브랜치로 push 한다

### #5 녹음, 기기 안 받아쓰기, 수정 화면을 만든다 (앱의 첫 쓸모)
- 상태: 대기
- 폴더: `project_manage/0005-2026_11_19-record_transcribe_edit`
- 작업: 없음
- 선행: #3 받아쓰기 엔진 interface(대기), 후속: 뒤의 학습 엔진 이슈
- 막힌 이유: #3 의 `Transcriber` interface 가 있어야 한다
- 다음 행동: #3 merge 뒤 app worker 에게 맡긴다

## 5. 순서
### 5-1. 지금 진행하는 이슈와 고른 이유
- #1 과 #2, 이유: 둘 다 선행이 없고 뒤의 모든 이슈가 이 둘에 기댄다, #1 은 Rust 갈래, #2 는 PyTorch 갈래라 파일이 겹치지 않는다
### 5-2. merge 순서와 얽힘
- 두 갈래가 함께 고치는 파일은 맨 위 `Makefile`, `.gitignore`, `README.md`, `.github/workflows/` 다, #1 이 맨 위 `Makefile` 과 CI 를 만들고 #2 는 `research/Makefile` 만 만든다, 맨 위 `Makefile` 에 research target 을 잇는 일은 먼저 merge 된 쪽 위에서 나중 쪽이 한다
### 5-3. 자리가 나면 띄울 순서
1. #3 받아쓰기 엔진 interface 와 속도 측정, #1 merge 뒤 app worker 자리
2. #4 라벨 없는 FL 모의실험, #2 merge 뒤 research worker 자리
3. #5 녹음과 받아쓰기와 수정 화면, #3 merge 뒤 app worker 자리
### 5-4. 새 이슈 후보
- 화면의 CSP(내용 보안 정책)를 `null` 에서 좁힌다: 앱 뼈대는 `null` 로 두었다, 녹음, 받아쓰기, 수정 화면을 만드는 이슈가 화면에 기능을 붙이기 전에 한다, 나온 곳 앱 뼈대 이슈 board 의 D9
- 기기 안 학습 엔진: 얼린 몸통 위의 작은 부분만 학습하는 `Trainer`, Rust 자동 미분 엔진(Burn, Candle) 고르기, 나온 곳 PD8
- FL 서버와 통신 규약: 업데이트 모으기, 공개 라벨 데이터 검증 관문, 나온 곳 PD8
- 폰 가속기로 받아쓰기: Android 와 iOS 의 GPU, NPU 엔진, 나온 곳 PD7
- 두 계층 사이 증류: 데스크톱 계층 모델에서 폰 계층 모델로, 나온 곳 PD6
- 화자 구분과 요약, 나온 곳 PD10
- 서명과 배포: Apple 과 Windows 인증서, 앱 장터, 나온 곳 #1 제외 범위
- 개인정보 안내와 FL 참여 동의 화면, 나온 곳 PD8

## 6. 사용자에게 올릴 것
### 6-1. 확인이 필요한 것 (잠정으로 정하고 진행한 것)
- 앱 identifier `io.github.guny524.seamone`(앱 뼈대 이슈 board 의 D4), 임시 아이콘(말미잘 모양, `app/src-tauri/app-icon.png`): 앱 장터에 올리기 전까지 바꿀 수 있다, 추천은 그대로
- PD2: 라이선스를 Apache-2.0 으로 정했다
- PD3: 기록을 git 에 두고 공개한다
- PD7: 속도 기준 숫자(데스크톱은 가속기로 실시간의 3배와 CPU 만으로 실시간, 폰은 가속기로 실시간)
- PD8: 라벨 없이 키우는 방향 여섯 가지와 안전장치
### 6-2. 아직 정하지 않은 것 (기다리는 것)
- main 이력의 작성자 메일: 첫 두 커밋(98ccde9, f17c964)의 작성자 메일이 회사 메일이라 공개 레포에 드러나 있다, 06:09 부터 이 레포의 새 커밋은 GitHub noreply 주소를 쓴다, 이미 push 한 이력을 고치려면 main 에 force push 가 필요해 사용자 승인이 필요하다, 추천: 레포가 새것이라 지금 고치는 것이 가장 싸다(그래도 GitHub 에 이미 남은 사본은 지워지지 않을 수 있다), 답이 없으면 그대로 둔다
- 큰 학습에 쓸 GPU: 집 서버를 써도 되는지와 클라우드 예산, 2026-11-19 에 물었고 답을 받지 못했다, 오늘 밤은 노트북만 쓴다, 직접 설계한 모델의 증류는 이 답 뒤에 한다
- PD9: 시작 모델, #2 와 #3 의 측정을 기다린다
- 서명: iOS 실기기 설치와 앱 장터 배포에는 사용자의 Apple 개발자 계정이 필요하다
### 6-3. 보고만 하는 것 (원칙에서 바로 나와 메인이 정한 것)
- PD5: Rust, Tauri 2, Leptos
- PD10: 첫 판은 녹음, 받아쓰기, 수정까지
- PD11: 실험은 PyTorch, 앱은 Rust

## 7. 사람 시험 대기
- 없음
