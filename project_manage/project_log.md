# seamone 프로젝트 log
(형식은 `project-issue-conductor` skill 의 `references/record-format.md` 7절, 시각 순으로 쌓고 지우지 않는다)

## 2026-11-19 04:10 메인(Fable 5.2 xhigh) 중단과 재개
- 프로젝트를 시작했다, 사용자가 GitHub 에 빈 공개 레포를 만들고 이 노트북에 clone 했다
- 사용자와 정한 것은 `project_decision.md` PD1~PD12, 사용자 원문은 `project_board.md` 2절
- GitHub 이슈 #1~#5 를 만들었다, 순서와 고른 이유는 `project_board.md` 5절
- 밤샘 실행(2026-11-19 09:00 까지)의 값은 `project_board.md` 3절, 사용자에게 물은 다섯 가운데 큰 학습용 GPU 하나는 답을 받지 못해 6-2 에 올렸다

## 2026-11-19 04:17 메인(Fable 5.2 xhigh) 사용자 전달
(밤샘 실행 절차다, 2026-11-19 09:00 까지 메인이 깨어날 때마다 이 순서를 따른다, 값의 원본은 `project_board.md` 3절이다)
### 1. 지금 떠 있는 것
- app worker: #1 앱 뼈대와 CI, Sonnet max, worktree `/Users/min-jo/orca/workspaces/seamone/1-app-skeleton-ci`, 브랜치 `guny524/1-app-skeleton-ci`, 터미널 `term_b522cdf5-deb1-4bf0-a0ca-285d2f4bc264`, 세션 `2e24423d-9a31-4d8f-8f08-04d4faaad311`, 04:12 시작
- research worker: #2 오류율 측정, Sonnet max, worktree `/Users/min-jo/orca/workspaces/seamone/2-start-model-cer`, 브랜치 `guny524/2-start-model-cer`, 터미널 `term_f05b49b0-2a37-4baf-95d0-b68df26ce2b7`, 세션 `0ee282a2-407f-4aab-8af7-8d53d330a854`, 04:12 시작
- 메인 터미널: `term_7c562cef-6922-4364-bf2f-5ff5629c97c2`, 활성 worker 목록은 `.tmp/conductor/workers.txt`, 점검마다의 상태 줄은 `.tmp/overnight/plan.md`
- 잠들지 않게 하는 `caffeinate -dims -t 19000` 이 돌고 있다(09:30 쯤 스스로 끝난다)
### 2. 깨어날 때마다 하는 순서
1. `date '+%H:%M'`, `df -h /Users/min-jo`, `/Users/min-jo/llms/skills/orchestration-cross-cli-review/scripts/orca_check_workers.sh /Users/min-jo/Documents/Developments/seamone/.tmp/conductor` 를 보고 `.tmp/overnight/plan.md` 에 한 줄을 더한다
2. worker 가 창에서 멈췄으면(`dlg=1`, `blocked`) 화면을 읽고 skill `dispatch-worker` 4-1절대로 답한다, 터미널이 끝났으면(`exited`) 그 worktree 에서 `claude --resume <세션> --model sonnet --effort max --permission-mode bypassPermissions` 로 탭을 다시 열고 이어 시킨다
3. 새 보고 파일(이슈 폴더의 `.tmp/report-<번호>.md`)이 있으면 아래 3절대로 확인한다
4. 둘 다 작업 중이고 보고가 없으면 아무것도 고치지 않고 한 줄로 끝낸다
5. 디스크 여유가 15GB 아래면 worker 에게 새로 받기를 멈추게 하고 `.tmp/hf` 에서 지금 쓰지 않는 가중치와 데이터를 지운다(지우기 전에 worker 가 쓰는 중인지 확인한다)
6. 답의 끝에 "물을 것: 있음 N개" 를 적고 `project_board.md` 6-2 의 항목을 맥락, 추천, 답이 없을 때 하는 일과 함께 다시 꺼낸다
### 3. 보고를 받으면
1. 보고 파일, 그 이슈 board 의 7절 체크와 8절, `issue_log.md` 를 읽는다
2. worktree 에서 `git status --short`, `git diff --stat` 을 보고, 그 이슈의 make target 을 메인이 직접 다시 돌린다(#1, #3, #5 는 `make lint test`, #2, #4 는 `make -C research lint test`), research 결과는 한 모델을 골라 저장된 가설 글로 CER 을 다시 계산해 대조한다
3. 맞으면 커밋한다: skill `git-commit` 3-1절대로 파일을 골라 `git add <파일>`, 메시지를 `validate_commit_message.py` 로 확인하고 커밋, `git push -u origin <브랜치>`, 첫 push 뒤 `gh pr create --base main --head <브랜치> --title "<이슈 제목>" --body "Refs #<번호>"`
4. #1 은 push 뒤 CI 가 돈다: `gh run list --branch <브랜치> --limit 5` 로 run id 를 보고 `gh run watch <id> --exit-status` 를 background 로 걸어 끝나면 깨어난다, 실패하면 app worker 세션에 "CI run <id> 가 실패했다, 7-10 대로 로그를 읽고 고쳐라, 보고 파일은 report-<다음 번호>.md" 를 보낸다(탭이 닫혔으면 세션을 다시 연다)
5. worker 에게 일을 보낼 때마다 그 응답 안에서 `orca_wait_worker.sh <터미널> <새 보고 파일> <최대 분>` 을 worker 마다 따로 background 로 건다
6. 보고를 받아들인 turn 에 그 worker 탭을 `orca_close_worker.sh /Users/min-jo/Documents/Developments/seamone/.tmp/conductor <이름>` 으로 닫는다, 곧 이어 시킬 것이면 닫지 않고 같은 세션에 다음 지시를 보낸다
7. 이슈의 칸이 다 끝나면 검토한다: 먼저 skill `review` 와 skill `orchestration-cross-cli-review` 의 `SKILL.md` 2-1절을 읽고 Codex Sol high 로 그 브랜치의 변경을 검토시킨다(검토자에게는 의도를 주지 않고 worktree 절대경로와 기준 commit 만 준다), 지적은 원문을 열어 맞는 것만 그 이슈 board 7절에 칸으로 더해 같은 worker 가 고친다, 고친 뒤 make target 을 다시 돌린다
8. skill `project-issue-conductor` 5절의 커밋 직전 증거 대조(이슈 board 10-2)를 메인이 직접 하고, 통과하면 `gh pr merge <번호> --merge`(squash 하지 않는다, 정리 스크립트가 보통 merge 를 전제로 한다), main 작업 사본에서 `git pull --ff-only`, `orca_cleanup_worktree.sh <worktree> main /Users/min-jo/Documents/Developments/seamone/.tmp/conductor` 로 확인한 뒤 `--apply`, 원격 브랜치는 `git push origin --delete <브랜치>`
9. 이슈 마무리 코멘트를 `gh issue comment <번호>` 로 올리고(skill `project-issue-conductor` 의 `references/templates.md` 5절), 사람 시험이 없는 이슈(#2, #3, #4)는 `gh issue close` 하고 폴더를 `_done_` 으로 `git mv` 한다, 사람 시험이 있는 이슈(#1, #5)는 닫지 않고 `project_board.md` 7절에 올린다
10. `project_board.md` 4절과 그 이슈 board 9, 10절을 고치고 기록을 커밋해 push 한다(기록 커밋은 main 에 바로)
### 4. 다음 일을 띄우는 순서
- app worker 자리: #1 이 merge 되면 #3(받아쓰기 엔진 interface 와 속도 측정), #3 이 merge 되면 #5(녹음, 받아쓰기, 수정 화면)
- research worker 자리: #2 가 merge 되면 #4(라벨 없는 FL 모의실험)
- 띄우는 법은 #1, #2 를 띄운 것과 같다: `orca worktree create --name <번호>-<이름> --repo path:/Users/min-jo/Documents/Developments/seamone --base-branch main --setup skip --no-parent --json`, 저절로 생긴 "Terminal 1" 탭을 닫고, `orca terminal create --worktree path:<worktree> --title "worker <이름> #<번호>" --command "env WORKER_MODEL=sonnet WORKER_EFFORT=max /Users/min-jo/llms/skills/orchestration-cross-cli-review/scripts/claude_worker_wrapper.sh" --json`, handle 을 `.tmp/conductor/workers.txt` 에 `<번호>-<이름> <handle>` 로 적고, 시작 지시 파일은 `project_manage/0001-2026_11_19-app_skeleton_ci/.tmp/start-1.md` 를 본떠 그 이슈 폴더의 `.tmp/start-1.md` 로 쓰고, 짧은 메시지로 그 파일을 가리킨다
- #3 의 속도 측정(7-4)과 #4 의 학습이 같은 노트북에서 겹치면 속도 수치가 틀어진다: #3 의 시작 지시에 "7-3 까지 하고 보고한 뒤 멈춘다" 를 넣고, 메인이 research worker 가 방법 하나를 끝내고 쉬는 때(없으면 "지금 방법이 끝나면 멈추고 보고해라" 를 보내서)에 7-4 를 시킨다, 측정이 끝나면 research worker 를 이어 시킨다
- Sonnet 한도가 끝나 worker 가 멈추면 같은 일을 Gemini 3.8 Flash high(`agy_worker_wrapper.sh`, `WORKER_AGY_SANDBOX=off`, 이슈 폴더와 `.tmp/hf` 를 `WORKER_AGY_ADD_DIRS` 로)로 넘기고 이 log 에 시각을 적는다
### 5. 메인이 정하는 것과 정하지 않는 것
- 정한다: 이슈 board 와 `project_decision.md` 안에서 나오는 구현 선택, worker 의 질문 가운데 기존 결정에서 바로 나오는 것(그 이슈 `issue_decision.md` 에 결정자 메인으로 적고 `project_board.md` 6-3 에 올린다)
- 시작 모델(PD9): #2 와 #3 의 결과가 모두 나오면 PD7 의 기준으로 계층마다 고르고, Codex Sol high 와 Gemini 에게 의견을 받아 `project_decision.md` 에 "[밤사이 결정]" 으로 근거와 함께 적는다, 앱에 고정하는 것은 #5 에서 한다
- 정하지 않는다: `project_board.md` 3절의 유보한 단계(메일과 Slack, 집 서버와 유료 클라우드, 서명과 배포, 레포 설정), 합의한 PD 의 뜻을 바꾸는 것, 이런 것은 `project_board.md` 6-2 에 올리고 그 결정에 기대지 않는 일을 계속한다
- 새 이슈는 만들지 않는다, 후보는 `project_board.md` 5-4 에만 올린다
### 6. 끝내기 (08:40 이후에 깨어났거나 일이 다 끝났을 때)
1. 새 일을 띄우지 않는다, 돌고 있는 worker 세션은 닫지 않는다(보고가 와 있으면 3절대로 확인까지 한다)
2. 끝난 일의 임시 파일을 지운다: 다음에 쓰지 않는 `.tmp/hf` 의 가중치와 데이터, 끝난 이슈의 `.tmp/`, merge 된 worktree, 아직 도는 worker 가 쓰는 것은 남기고 무엇을 왜 남기는지와 지울 조건을 적는다
3. 이 log 에 아침 보고를 쓴다(skill `overnight-autopilot` 4절): 밤새 한 일, 커밋과 push 와 merge 와 close 의 hash 와 제목, [밤사이 결정] 과 근거, 보류 목록과 추천, 멈춘 곳과 까닭, 남은 일, 쓴 모델과 바꾼 때, 끝낸 것과 확인한 것과 확인하지 못한 것을 나눠서
4. 기록을 커밋해 push 하고, `CronList` 로 본 점검 cron 을 `CronDelete` 로 지운다
5. 모델 설정을 되돌린다: `.tmp/overnight/settings-before-switch.tsv` 의 값(모델 `claude-fable-5-2`, Fable effort `xhigh`, Sonnet effort `medium`)대로, 자기 터미널에 `orca terminal send --terminal "$ORCA_TERMINAL_HANDLE" --text "/effort medium" --enter` 를 보내 Sonnet 의 effort 를 되돌린 뒤 `--text "/model claude-fable-5-2" --enter` 를 보내고, 화면에 "Switch model?" 이 뜨면 `--text $'\r'` 로 확인하고, `--text "/effort xhigh" --enter` 를 보낸다, `jq '{model, modelSettings}' ~/.claude/settings.json` 으로 세 값이 돌아왔는지 확인한다

## 2026-11-19 06:20 메인(Sonnet max) 점검
- 상태: app worker 가 첫 보고(`project_manage/0001-2026_11_19-app_skeleton_ci/.tmp/report-1.md`)를 썼다, research worker 는 LoRA 시험을 쉬는 중이고 보고 없음
- 메인이 직접 확인한 것: 06:07 worktree 에서 `CARGO_BUILD_JOBS=1 nice -n 10 make lint` EXIT=0, `make test` EXIT=0(시험 4개 통과: core 3개와 command 1개), 커밋 대상 77개 파일에 `.tmp`, `target`, `gen`, `dist` 와 비밀값이 섞이지 않음
- 한 일: 커밋 24ff6d2(`feat: Tauri 2 앱 뼈대와 다섯 플랫폼 설치 파일 CI 를 추가`, 77개 파일)를 브랜치 `guny524/1-app-skeleton-ci` 에 push, draft PR https://github.com/guny524/seamone/pull/6 을 열었다(`build.yml` 이 pull_request 에서만 돌아서), CI 세 개(ci push, ci pull_request, build pull_request)가 06:10 에 시작했다
- 정한 것: 노트북 메모리가 한계라 로컬 `make build` 와 `cargo tauri dev` 는 하지 않고 설치 파일은 CI 로만 확인한다(프로젝트 PD4 에서 나온 결정, 메인), 이 이슈 완료 기준의 로컬 `make build` 와 창 확인은 CI artifact 와 사람 시험으로 대신한다
- 고친 것: 이 레포의 커밋 작성자 메일을 회사 메일에서 GitHub noreply 주소로 바꿨다(레포 로컬 설정), 이미 push 한 두 커밋은 그대로이고 사용자에게 올린다(project_board 6-2)
- 실수: 06:15 의 커밋 e6ea2be 는 메시지(`docs: 앱 뼈대 보고 확인과 CI 시작을 기록에 적음`)와 달리 두 worker 가 쓰던 이슈 기록(두 이슈의 issue_board, issue_decision, issue_log)만 담았다, 기록 편집 스크립트가 중간에 멈췄는데 뒤 명령(커밋과 push)이 그대로 실행돼서다, 내용은 worker 의 정상 기록이라 되돌리지 않고 이 항목으로 바로잡는다
- 다음: app worker 가 CI 로그를 읽고 고쳐 report-2.md 를 쓰면 메인이 확인해 push 한다, 06:22 에 CI 결과를 한 번 본다(일회성 cron), Claude Code 가 메모리 부족으로 메인의 background 대기 둘을 멈춘 뒤 새 대기는 걸지 않았다

## 2026-11-19 06:28 메인(Sonnet max) 검토
- 상태: GitHub Actions 세 개(ci 의 push 와 pull_request, build 의 pull_request)가 모두 성공했다, build 의 job 6개(ios, android, desktop 의 windows-x86_64, macos-arm64, macos-x86_64, linux-x86_64)가 모두 성공이고 건너뛴 단계는 macOS 와 Windows 의 "Install the Tauri Linux dependencies" 뿐이다(기대한 것)
- 메인이 직접 확인한 것: 받은 artifact 의 이름, 크기, 형식 — Android `app-universal-debug.apk` 49.8MB(zip), iOS `seamone-ios-simulator.zip` 14.6MB, macOS `seamone_0.1.0_aarch64.dmg` 2.1MB 와 `seamone_0.1.0_x64.dmg` 2.3MB, Windows `seamone_0.1.0_x64_en-US.msi` 2.1MB 와 `seamone_0.1.0_x64-setup.exe`(NSIS) 1.4MB, Linux 78MB 는 내려받는 중이라 이름과 형식은 아직 확인하지 못했다
- 한 일: 검토를 시작했다, 검토자는 Codex Sol(`gpt-6.1-sol`) high 읽기 전용(`-s read-only`), 의도를 주지 않고 변경과 레포만 보게 했다, 시작 전 worktree 의 미커밋 변경은 0 줄(스냅샷 `.tmp/review/status-before-1.txt`), 대상은 커밋 24ff6d2 하나(77개 파일, Cargo.lock 5416줄 포함)
- 정한 것: 검토자를 Claude 의 background 명령이 아니라 Orca 터미널에 띄웠다(Claude Code 가 메모리 부족일 때 background 명령을 멈추므로), 결과는 파일로 받고 다음 점검에서 읽는다
- 다음: 검토 결과를 읽고 지적마다 원문을 열어 맞는 것만 이슈 board 7절에 칸으로 더한다, 검토 지적을 고친 뒤에는 lint 와 test 를 다시 돌리고 CI 를 다시 보고 재검토는 두 번까지

## 2026-11-19 06:58 메인(Sonnet max) 검토
- 검토: Codex Sol(`gpt-6.1-sol`) high 읽기 전용, 06:25 시작 06:30 끝, 앞뒤 미커밋 변경 같음(스냅샷 `.tmp/review/status-before-1.txt` 와 `status-after-1.txt`), 대상은 커밋 24ff6d2 하나, 원문은 `.tmp/review/codex-1.md`(세션 `01a1f08d-4843-7021-a551-da6656dd58f7`), 검토자는 clippy 와 시험을 sandbox 쓰기 거부로 돌리지 못했고 서식 검사는 통과했다고 보고했다
- 지적 네 개를 메인이 원문을 열어 모두 맞다고 확인했다
  - 낮음, 방향: CI 의 desktop job 은 `cargo tauri build` 를 직접 부르고(`.github/workflows/build.yml:77`) `Makefile` 의 `build` target 은 CI 가 쓰지 않으며 README 3-2 는 로컬 설치 파일 빌드를 안내해 프로젝트 결정 PD4 와 어긋난다 -> 받음, CI 가 `make build` 를 부르게 하고 README 를 고친다
  - 중간, 결함: `scripts/install-tools.sh` 가 받은 압축 파일의 해시를 확인하지 않고, 두 workflow 의 `uses:` 12곳이 이동 가능한 태그다 -> 받음, 해시 확인과 commit SHA 고정
  - 중간, 결함: Android CI 의 debug 서명 키가 실행마다 새로 생겨 덮어 설치가 거부될 수 있다 -> 받음(추론이라 재현은 못 한다), `actions/cache` 로 키를 고정하고 두 번 돌려 확인
  - 낮음, 결함: command 시험이 `generate_handler!` 등록 누락과 화면의 command 이름 오타를 잡지 못한다 -> 받음, command 이름 상수 하나와 `tauri::test` 로 IPC 시험, 일부러 어긴 구현으로 시험이 실패하는지도 본다
- 해당 없음으로 둔 지적: 없음
- 처리: 네 칸을 `project_manage/0001-2026_11_19-app_skeleton_ci/.tmp/review-cells-1.md` 에 쓰고 app worker 에 보냈다(이슈 board 7절 끝에 7-12 부터 7-15 로 옮겨 적어 차례로 고치고 report-2.md 를 쓴다), 검토자 터미널 탭은 닫았다
- 다음: report-2.md 를 확인해 커밋과 push, CI 확인(Android 키 고정은 `gh run rerun` 으로 두 번째 실행까지), 재검토 한 번(두 번까지)

## 2026-11-19 07:58 메인(Sonnet max) 점검
- 상태: 두 worker 가 보고를 썼다, app worker 의 두 번째 보고(`project_manage/0001-2026_11_19-app_skeleton_ci/.tmp/report-2.md`)와 research worker 의 첫 보고(`project_manage/0002-2026_11_19-start_model_cer/.tmp/report-1.md`)
- 앱 뼈대(app worker 보고 2): 검토 지적 네 개를 파일 8개에 고쳤다, 메인이 직접 확인한 것: worktree 에서 `make lint`, `make test` EXIT=0(시험 4개, IPC 시험 포함), build.yml 과 install-tools.sh 의 diff 를 읽었고, 고정한 action SHA 다섯 개(checkout, upload-artifact, setup-java, rust-cache, cache)가 각 이동 태그의 현재 commit 과 같음을 `gh api` 로 대조했다, 커밋 4c4d2e4(`fix: 설치 도구 해시 확인, action SHA 고정, Android 키 고정, IPC 시험 추가`)를 push 해 CI 가 07:52 에 다시 시작했다
  - worker 가 CI 가 만든 macOS 앱을 이 노트북에서 실행해 첫 화면(`seamone`, `seamone 0.1.0`, 화면이 IPC 로 command 를 불러 받은 값)을 확인했다고 보고했다(캡처는 이슈 폴더 `.tmp/macos-arm64-app-window.png`), 이 확인은 worker 주장이고 메인이 직접 확인하지는 않았다
  - worker 가 정리하다 메인이 쓴 `.tmp/start-1.md` 를 `rm -f` 로 지웠고 같은 글로 다시 썼다고 스스로 보고했다
  - 남은 CI 확인: Windows 의 `choco install make` 와 `make build`, 해시 확인이 세 OS 에서 통과하는지, Android 키 고정의 두 번째 실행(`gh run rerun`)
- 시작 모델 오류율(research worker 보고 1): 메인이 직접 확인한 것: `make -C research lint`, `make -C research test` EXIT=0(51개 통과), 저장된 가설 글에서 내 독립 코드(레포 코드를 쓰지 않음)로 센 오류율이 보고와 같다(Qwen3-ASR-1.7B 한국어 56개 4.53%, whisper-large-v3-turbo 5.83%, whisper-small 앞 56개 7.65%)
  - 결과(worker 보고, 같은 한국어 발화 64개 합산): Qwen3-ASR-1.7B 4.19%, whisper-large-v3-turbo 5.44%, whisper-small 7.47%, whisper-base 13.89%, 영어 40개는 1.78%, 2.42%, 3.68%, 5.30%, 1.7B 와 turbo 의 차이가 표준오차의 약 2.7배라 확정은 아니다, LoRA 는 Apple GPU 에서 글 디코더만 붙이는 범위로 whisper-small, turbo, Qwen3-ASR-1.7B(묶음 1) 가 돌았고 Qwen3-ASR-1.7B 묶음 4 는 GPU 메모리 한도를 넘었다
  - worker 가 스스로 적은 규칙 이탈: 05:58 지시와 달리 whisper-small LoRA 시험을 06:20 까지 돌렸고(멈춘 시각 06:20), 모델 실행을 겹쳐 돌렸으며, 시간 한도를 넘겼고, 지시한 표본보다 적게 쟀다(부하와 디스크 때문), 메인은 근거가 메모리 부족의 실제 신호와 맞아 받아들였고 앞으로는 이탈하기 전에 근거를 board 4-2 에 먼저 적게 했다
  - worker 가 `~/.cache/uv/archive-v0` 에서 쓰는 중이 아닌 항목 487개를 지워 uv 캐시가 9.8GB 에서 1.3GB 로 줄었다(memory 의 다시 받을 수 있는 캐시 규칙, 쓰는 중인 6개는 `ps` 로 가려 두었다)
  - 이슈 board 2-3절의 "Whisper 는 MIT" 를 모델 카드 license 태그대로 고쳤다(large-v3-turbo 만 MIT, base, small, large-v3 는 Apache-2.0)
  - 커밋 65bb8c4(`feat: 시작 모델 후보의 오류율 측정 코드와 결과 문서를 추가`, 새 파일 26개)를 push 하고 draft PR https://github.com/guny524/seamone/pull/7 을 열었다, 이 브랜치에는 workflow 가 없어 CI 가 돌지 않는다
- 정한 것(메인): 표본을 더 늘리는 일(Zeroth 60개)은 부하가 낮을 때로 미룬다(다음에 돌릴 명령은 보고에 있다), Qwen3-ASR-0.6B 와 whisper-large-v3 는 받지 않는다, 시작 모델은 속도 측정을 기다려 결정한다(프로젝트 PD9)
- 한 일: Codex Sol high 읽기 전용 검토를 시작했다(07:55, 결과 `.tmp/review/codex-2.md`, 시작 전 스냅샷 `.tmp/review/status-before-2.txt`), research worker 에게 다음 일(라벨 없는 FL 모의실험, 작은 규모, 08:35 까지, 파이프라인을 끝까지 돌려 보는 목적)을 보냈다(`project_manage/0004-2026_11_19-unlabeled_fl_sim/.tmp/start-1.md`)
- 실수: 앱 뼈대 커밋을 처음 시도할 때 zsh 가 파일 목록 변수를 낱말로 나누지 않아 `git add` 가 실패했다, 뒤 명령은 `&&` 로 이어져 커밋과 push 는 일어나지 않았고 파일 이름을 직접 적어 다시 했다

## 2026-11-19 08:12 메인(Sonnet max) 점검
- 앱 뼈대 CI 확인(일회성 점검): 고친 커밋 4c4d2e4 의 `ci` 둘과 `build`(job 6개)가 모두 성공했고 artifact 6개가 다시 올라왔다, 메인이 직접 확인한 것
  - 7-12(CI 가 `make build` 를 부름): Windows 에서 `choco install make --version=4.4.1` 이 되고 `make build TAURI_BUILD_ARGS="--target x86_64-pc-windows-msvc --bundles nsis,msi"` 가 `cargo tauri build` 를 돌려 `release` 빌드가 끝났다, macOS, Linux 도 같은 명령으로 통과
  - 7-13(고정): 해시 확인을 통과해 세 OS(macOS, Linux, Windows)에서 도구가 설치됐다(`설치했다:` 줄), `uses:` 13곳은 이동 태그의 현재 commit 과 같은 SHA 로 고정돼 있다
  - 7-14(Android 서명 키): 첫 실행은 `keystore cache-hit: ''`(miss)에 키 2618바이트가 생겼고 sha256 `0dea05701b58e9e2483b0635b5a0ecbcd0b2b2516c890d91269a39d6e2c8a578`, 같은 run 을 `gh run rerun` 으로 다시 돌린 두 번째 실행은 `cache-hit: 'true'` 에 같은 파일 시각과 같은 sha256 이다, 이 키는 시험용이고 release 에 쓰지 않는다는 주석이 workflow 에 있다
  - 7-15(IPC 시험): 맨 위 `make test` 가 Ubuntu CI 에서 통과(첫 CI 는 macOS 로컬 시험으로만 확인했던 것), 일부러 어긴 구현에서 시험이 실패하는 것은 worker 가 로컬에서 보였다
- 앱 뼈대 재검토: Codex Sol high 읽기 전용 두 번째 검토를 08:07 에 시작했다(이전 지적 네 개의 건별 판정과 새 지적을 요청, 결과 `.tmp/review/codex-3.md`, 시작 전 스냅샷 `.tmp/review/status-before-3.txt`)
- 시작 모델 오류율 검토 결과(Codex Sol high, 07:55 시작 08:03 끝, 원문 `.tmp/review/codex-2.md`, 검토자는 `make -C research test` 를 sandbox 의 uv 잠금 파일 쓰기 거부로 돌리지 못했다고 보고했다)
  - 지적 여섯 개를 메인이 코드 원문을 열어 모두 맞다고 확인했다
    - 중간, 결함: `data.py:158-161` 의 `_drop_hub_cache` 가 공유 `HF_HOME/hub/datasets--<저장소>` 폴더를 통째로 지운다(같은 저장소의 다른 파일과 다른 실행이 받은 학습 조각까지, 지금 진행 중인 라벨 없는 FL 모의실험이 Zeroth-Korean 학습 조각을 받으면 겹친다)
    - 중간, 결함: `hypotheses.py:10-13` 이 중단으로 마지막 줄이 잘린 가설 파일을 못 읽어 재개가 막힌다
    - 중간, 결함: `lora_probe.py:161-169` 가 기울기가 없는 가중치와 비유한 손실을 성공으로 본다
    - 중간, 결함: 모델 판(결과 문서 24-26줄에 적힌 커밋)을 `transcribe.py`, `prefetch.py`, `lora_probe.py` 의 로더가 쓰지 않아 재실행 결과가 섞일 수 있다
    - 중간, 결함: `data.py:260-266` 의 음성 cache 가 데이터 판과 표본을 검사하지 않고 존재만 본다
    - 낮음, 방향: 결과 문서 185줄의 "되풀이하는 환각은 없었다" 가 두 배 길이 검사만으로 단정한다(`score.py:30-35`)
  - 처리: 여섯 개 모두 받음(해당 없음으로 둔 것 없음), `.tmp/review-cells-2.md` 로 research worker 에게 보냈다(이슈 board 7절 끝에 7-9 부터 7-14), 지금 하는 라벨 없는 FL 모의실험보다 먼저 08:30 까지 고치게 했고 그 일의 한도는 08:50 으로 늘렸다
  - 검토자의 도구(Serena)가 worktree 에 만든 `.serena/` 폴더는 지웠다(커밋에 들어가지 않았다)
- 받아쓰기 엔진 이슈: app worker 에게 앞 절반(interface 와 가짜 엔진과 엔진 후보 조사와 순수 함수)을 08:00 에 보냈다(앞 뼈대 이슈와 같은 worktree, 무거운 일 없이 08:35 까지, 새 파일은 별도 브랜치 `guny524/3-asr-engine-speed` 로 메인이 커밋한다)
- 라벨 없는 FL 모의실험: research worker 가 08:08 까지 `fedavg.py`, `fl_data.py`, `fl_sim.py`, `fl_train.py`, `pseudo_label.py` 와 시험 4개 파일을 만들었다(새 파일, 아직 안 읽음)

## 2026-11-19 08:42 메인(Sonnet max) 점검
- 앱 뼈대: 두 번째 검토(08:07 시작 08:13 끝) 결과를 읽었다, 이전 지적 네 개는 고쳐짐 셋과 일부 하나, 새 지적 둘(낮음 README, 중간 Android 키 보존)을 원문으로 확인해 모두 받았다
  - 고친 것: README 의 `make build` 설명을 데스크톱으로 한정(커밋 c2c9e91), Android debug 서명 키를 캐시가 아니라 레포 `.github/android/debug.keystore` 로 두었다(커밋 2337513, 키는 첫 CI 가 만든 것을 artifact 로 받아 커밋, sha256 dba5f1e5d6cd77045250f3d641da991ed6de0010c9a1ea422ba5be980d8aa08d), workflow 는 빌드 뒤 APK 를 올리기 전에 `cmp` 로 서명에 쓰인 키가 레포 키와 같은지 확인한다
  - 메인이 직접 확인한 것: 마지막 CI(`ci` 둘, `build` 의 job 6개)가 모두 success, Android job 로그에 `committed keystore: 'true'` 와 레포 키와 같은 sha256, 이슈 board 10-2 증거 대조 통과(사람 시험 대기)
  - PR https://github.com/guny524/seamone/pull/6 을 ready 로 바꿔 merge 했다(merge 커밋 8f7e917, 2026-11-19 08:36), main 작업 사본은 `git pull --ff-only` 로 맞췄다, GitHub 이슈에 진행 요약 댓글을 올렸다
  - 실수: 시험용 키 artifact 를 받는 명령이 폴더가 없어 한 번 실패했고 뒤 명령은 `&&` 로 멈춰 아무것도 커밋되지 않았다(폴더를 만들고 다시 받았다)
- 받아쓰기 엔진: app worker 의 첫 보고(08:22)를 확인했다, 메인이 직접 확인한 것: `make lint`, `make test` EXIT=0(시험 27개 통과, worker 가 새로 쓴 것은 23개), 커밋 e763890(`feat: 받아쓰기 interface, 전사 조각 타입, 가짜 엔진, 실시간 배수 계산을 추가`, 8개 파일)를 별도 브랜치 `guny524/3-asr-engine-speed` 에 push 하고 draft PR https://github.com/guny524/seamone/pull/8 을 열었다, 엔진 후보 조사 결과(whisper-rs 와 qwen-asr 를 잠정 선택, Candle 은 대안)는 worker 가 조회한 값이고 메인이 재조회하지는 않았다
- 시작 모델 오류율: research worker 의 보고 2(08:24)를 확인했다, 메인이 직접 확인한 것: `make -C research lint`, `make -C research test` EXIT=0(111개 통과), 삭제 범위 검사(`remove_inside`)와 잘린 가설 파일 복구(`_parse`, `repair_hypotheses`) 코드를 읽었다, 커밋 6d642da(`fix: 측정 코드의 cache 삭제 범위, 판 고정, 재개와 LoRA 판정을 검토대로 고침`, 16개 파일)을 push 했다(PR 7 에 더해짐, 라벨 없는 FL 모의실험 몫의 변경은 뺐다)
  - 두 번째 검토(08:31 시작 08:36 끝): 이전 지적 여섯 개는 고쳐짐 넷과 일부 둘, 새 지적 셋(중간 셋: 세트별 cache 의 실행 간 배제 없음, 측정 설정 파일의 비원자적 쓰기, 표본 목록이 바뀌어도 설정 대조 통과)을 원문으로 확인했다, 세트별 cache 의 동시 실행 경쟁은 지금 한 worker 가 쓰는 환경에서는 드물지만 맞는 지적이다
  - 처리: 세 개를 모두 받아 `.tmp/review-cells-3.md` 로 research worker 에게 보냈다(08:50 까지), 검토 반복이 두 번에서 멈추는 규칙(review skill 4절)이라 세 번째 검토는 하지 않고 시험과 diff 로 메인이 확인한다
- 정리: 끝난 검토자 탭 셋을 닫았다, 끝난 일의 임시 사본은 지웠다(`.tmp/ci-artifacts`, 검토 도구가 만든 `.serena/`)

## 2026-11-19 08:55 메인(Sonnet max) 점검
- 시작 모델 오류율: research worker 의 보고 3(08:45)을 확인했다, 메인이 직접 확인한 것: `make -C research lint`, `make -C research test` EXIT=0(129개 통과), 세트별 잠금(`fcntl.flock`)과 `prepare_audio` 의 잠금 뒤 재확인 코드를 읽었다, worker 가 일부러 어긴 구현 아홉 가지에서 시험이 실패하는 것을 확인했다고 보고했고 메인은 이것을 다시 하지 않았다
- 커밋 398e2fb(`fix: 측정 코드의 준비 잠금, 설정 원자적 쓰기, 표본 목록 대조를 검토대로 고침`, 6개 파일)을 push 했다(README 는 이 이슈 몫의 한 줄만 `git apply --cached --unidiff-zero` 로 골랐다), 세 번째 검토는 하지 않았다(두 번에서 멈추는 규칙)
- PR https://github.com/guny524/seamone/pull/7 을 ready 로 바꿔 merge 했다(merge 커밋 7d069d4, 08:52), 이슈 마무리 코멘트를 올리고 이슈를 닫았고 폴더를 `_done_0002-2026_11_19-start_model_cer` 로 옮겼다(git 이 추적하지 않는 `.tmp/` 가 같이 가도록 `mv` 로 옮기고 `git add -A` 로 rename 을 잡았다)
- 앱 뼈대의 app worker 터미널은 Claude 프로세스가 끝나 있었다(할 일이 없는 상태라 다시 띄우지 않았다)

## 2026-11-19 08:58 메인(Sonnet max) 사용자 보고(아침 보고)
- 요점: 밤새(04:10 부터 08:58) 앱 뼈대와 시작 모델 오류율 측정 이슈를 merge 하고 끝냈다, 받아쓰기 엔진과 라벨 없는 FL 모의실험은 작은 규모의 앞 절반을 draft PR 로 올렸고, 노트북 메모리가 한계라 큰 일은 하지 않았다
- 이슈와 PR
  - 앱 뼈대(이슈 1): PR https://github.com/guny524/seamone/pull/6 merge(8f7e917), GitHub Actions 가 macOS dmg 둘, Windows exe 와 msi, Linux deb 와 AppImage, Android debug apk, iOS 시뮬레이터 zip 을 서명 없이 만든다(모두 CI 로 확인, 형식도 받아 확인), 사람 시험(설치 파일 실행)이 남아 이슈는 열려 있다
  - 시작 모델 오류율(이슈 2): PR https://github.com/guny524/seamone/pull/7 merge(7d069d4), 이슈 닫음, 결과는 `research/results/2026_11_19_start_model_cer.md`
  - 받아쓰기 엔진(이슈 3): 앞 절반(interface, 전사 조각 타입, 가짜 엔진, 순수 함수, 엔진 후보 조사)을 draft PR https://github.com/guny524/seamone/pull/8(커밋 e763890), 코드 검토 아직
  - 라벨 없는 FL 모의실험(이슈 4): 작은 규모 실행을 draft PR https://github.com/guny524/seamone/pull/9(커밋 0d93e7c), 코드 검토 아직
  - 녹음, 기기 안 받아쓰기, 수정 화면(이슈 5): 시작하지 못했다(받아쓰기 엔진 interface 가 먼저)
- 얻은 것(근거는 결과 문서, 표본이 작아 확정은 아니다)
  - 한국어 오류율(같은 발화 64개): Qwen3-ASR 1.7B 4.19%, whisper-large-v3-turbo 5.44%, whisper-small 7.47%, whisper-base 13.89%, 영어 40개는 1.78%, 2.42%, 3.68%, 5.30%, 앞 둘의 차이는 표준오차의 약 2.7배
  - 글 디코더만 붙이는 LoRA 는 Apple GPU 에서 whisper-small, turbo, Qwen3-ASR 1.7B(묶음 1)가 돈다, 업데이트 크기는 글 디코더 LoRA 한 벌 2.4MB
  - 라벨 없는 FL 모의실험은 파이프라인이 끝까지 돈다, 이 규모(기기 8, 발화 6, 라운드 2)로는 효과를 판정하지 못했다(차이가 모두 표준오차의 1.3배 이하), 큰 규모를 돌려야 한다
  - 시작 모델은 아직 정하지 않았다(프로젝트 PD9): 오류율은 Qwen3-ASR 1.7B 가 앞서고 속도는 받아쓰기 엔진 이슈의 뒤 절반에서 잰다
- [밤사이 결정](메인이 정하고 근거를 적은 것, 사용자 확인 대기는 project_board 6-1)
  - 프로젝트 결정 PD2(라이선스 Apache-2.0), PD3(기록을 git 에 두고 공개), PD5(Rust, Tauri 2, Leptos), PD7(속도 기준 숫자), PD8(라벨 없이 키우는 방향 여섯 가지), PD10(첫 판은 녹음과 받아쓰기와 수정까지), PD11(실험은 PyTorch), 근거는 각 결정 항목
  - 앱 뼈대의 로컬 `make build` 와 창 확인은 하지 않고 설치 파일은 CI 로만 확인(PD4 와 메모리 한계), Android debug 서명 키를 레포에 둠(시험용으로 공개된 키), 검토는 두 번에서 멈추고 메인이 시험과 diff 로 확인, background 대기 명령은 시스템이 메모리 부족으로 멈춘 뒤 다시 걸지 않고 점검 cron 으로 보고 파일을 직접 확인, 라벨 없는 FL 모의실험과 받아쓰기 엔진은 작은 규모와 앞 절반으로 줄임
- 사용자에게 묻고 답을 못 받은 것(project_board 6-2): 집 서버 RTX 3060 Ti 를 큰 학습에 써도 되는지(추천: 써도 된다, 답이 없어 노트북만 씀), 이미 push 한 두 커밋(158c9d3, 3169495)의 작성자 메일이 회사 메일이라 공개 레포에 드러난 것을 main 에 force push 로 바꿀지(추천: 새 레포인 지금 바꾼다, 답이 없어 그대로 둠, 06:09 부터 새 커밋은 noreply 주소)
- 검토: Codex Sol(`gpt-6.1-sol`) high 읽기 전용 네 번(앱 뼈대 둘, 시작 모델 둘), 지적 열다섯 개(낮음 넷, 중간 열하나)를 모두 원문으로 확인해 받아 고쳤다, 해당 없음으로 둔 것은 없다, 받아쓰기 엔진과 라벨 없는 FL 모의실험의 코드는 아직 검토받지 않았다
- 쓴 모델: 계획 Fable 5.2 xhigh(04:10 쯤까지), 이후 메인 Sonnet 5.5 max(04:18 에 바꿈), worker 둘 Sonnet 5.5 max, 검토 Codex Sol high, Gemini 로 넘기는 일은 없었다(worker 한도 소진 없음)
- 실수와 정정(숨기지 않는다)
  - 06:15 의 기록 커밋 e6ea2be 는 메시지와 달리 두 worker 의 이슈 기록만 담았다(편집 스크립트가 멈췄는데 뒤 명령이 커밋과 push 를 했다), 내용은 정상이라 되돌리지 않고 log 에 바로잡았다
  - 05:07 에 다시 받을 수 있는 Go, gradle, npm 캐시를 지울 때 도는 프로세스를 확인하는 명령과 지우는 명령을 한 번에 묶어, 다른 세션의 `go test` 와 gradle, kotlin 데몬이 쓰던 캐시까지 지웠다(go test 는 지운 뒤 끝났고 결과는 못 봤다, 데몬은 살아 있다)
  - 공개 레포의 첫 두 커밋 작성자 메일이 회사 메일이다(위 질문)
  - 마무리에서 이 셸의 `rm` 이 휴지통으로 보내는 별칭이라 지운 가중치와 빌드 폴더 12GB 가 휴지통에 들어갔고, 내가 넣은 네 항목만 정확한 이름으로 영구 삭제해 디스크 여유를 21GB 에서 33GB 로 되돌렸다
  - zsh 가 파일 목록 변수를 낱말로 나누지 않아 커밋 명령이 한 번 실패했고, 시험용 키 artifact 받기가 폴더가 없어 한 번 실패했다(둘 다 뒤 명령이 `&&` 로 멈춰 피해는 없었다)
  - worker 들이 스스로 보고한 것: app worker 가 정리하다 메인이 쓴 `.tmp/start-1.md` 를 지우고 같은 글로 다시 썼다, research worker 가 05:58 지시와 달리 LoRA 시험을 06:20 까지 돌렸고 모델 실행을 겹치고 시간 한도를 넘겼으며 표본을 지시보다 적게 쟀다(부하와 디스크 때문)
- 끝낸 것, 확인한 것, 확인하지 못한 것
  - 끝낸 것: 앱 뼈대 merge, 시작 모델 오류율 merge 와 이슈 닫음
  - 메인이 직접 확인한 것: 로컬 `make lint`, `make test` 와 `make -C research lint`, `test` 통과, CI 결과(여덟 번의 run 이 모두 success), 설치 파일 여덟 개의 형식, Android 키가 레포 키와 같은 sha256, 독립 코드로 센 오류율이 보고와 같음, crates.io 에서 엔진 crate 의 판과 라이선스
  - 확인하지 못한 것: 설치 파일을 실제 기기에서 실행해 첫 화면이 뜨는지(macOS 는 worker 가 CI 앱으로 확인했다고 보고했고 메인은 못 봄, 나머지 대기), 엔진 후보의 모바일과 Windows 빌드, 일부러 어긴 구현에서 시험이 실패하는 확인(worker 가 보고했고 메인은 다시 하지 않음), 라벨 없는 FL 모의실험의 효과, 받아쓰기 엔진과 FL 코드의 검토
- 남은 일(우선순위 순): 1) 앱 뼈대 사람 시험, 2) 받아쓰기 엔진 앞 절반 검토와 merge, 뒤 절반(엔진을 feature 뒤에 넣어 CI 빌드, 속도 측정, 시작 모델 결정), 3) 라벨 없는 FL 큰 규모 실행(약 3시간)과 코드 검토, 4) 녹음과 받아쓰기와 수정 화면, 5) research 의 시험을 CI 에서 돌리기, 서명과 배포, 개인정보 안내(project_board 5-4)
- 자원(마무리에서 지움): `.tmp/hf` 의 가중치 6.8GB, 앞 이슈 worktree 의 `target/` 4.6GB, 시작 모델 worktree 의 `research/.venv` 1.3GB 를 지웠다(다시 받거나 다시 만들 수 있다, 디스크 여유 33.5GB), 남긴 것: worktree 둘(`/Users/min-jo/orca/workspaces/seamone/1-app-skeleton-ci` 는 브랜치 `guny524/3-asr-engine-speed`, `2-start-model-cer` 는 `guny524/4-unlabeled-fl-sim` 를 쓴다, 그 PR 이 merge 되면 `dispatch-worker` skill 6절로 정리), 시험 음성 `.tmp/hf/seamone_eval` 169MB 와 가설 글(`.tmp/research/hyp/`, 다시 만들 수 없는 측정 증거), 이 레포의 `.tmp/review/` 의 검토 원문
- 점검 cron 과 일회성 점검을 모두 지웠고 모델 설정은 되돌렸다(아래 마지막 줄)
- 08:57 마무리 확인: 점검 cron 둘(주기 점검 90bfd2b1, 끝내기 일회성 cc9b85bd)을 지웠다, 모델 설정을 되돌렸다(`~/.claude/settings.json` 의 model `claude-fable-5-2`, Fable effort `xhigh`, Sonnet effort `medium`, 바꾸기 전 값과 같다), worker 탭 둘과 검토자 탭을 모두 닫았고 메인이 띄운 `caffeinate` 를 끝냈다, 08:58 로 걸어 둔 끝내기 일회성 점검은 그 전에 같은 일을 마쳐 지웠다

## 2026-11-19 09:25 메인(Opus) 사용자 전달
- 사용자 지시(2026-11-19): "이전 commit 도 edit 해 guny524@gmail.com 을 쓰게 local 에서 전부 바꾸고 push force 해"
- 한 일: 로컬 브랜치 다섯(main, guny524/1-app-skeleton-ci, guny524/2-start-model-cer, guny524/3-asr-engine-speed, guny524/4-unlabeled-fl-sim)을 `git filter-branch --env-filter` 한 번으로 다시 써서 23개 커밋의 작성자와 커미터 메일(`mingi.jo@zeroone.ai`, `43983919+guny524@users.noreply.github.com`)을 `guny524@gmail.com` 으로 바꾸고 `--force-with-lease` 로 force push 했다, 레포 로컬 설정 `user.email` 도 `guny524@gmail.com` 으로 바꿨다
- 확인한 것: 다섯 브랜치 모두 tip 의 tree 가 다시 쓰기 전과 같고(내용은 그대로, 메타데이터만 바뀜) 원격과 로컬이 같다, 원격 이력의 메일은 21개 커밋이 작성자와 커미터 모두 gmail 이고 PR merge 커밋 둘은 작성자가 gmail, 커미터가 GitHub 계정(`noreply@github.com`)이라 그대로 두었다
- 남은 노출: GitHub 이 옛 회사 메일 커밋 98ccde9 와 f17c964 를 hash 로 아직 보여 준다, 98ccde9 는 merge 된 PR 6, 7 의 읽기 전용 참조(`refs/pull/6/head`, `refs/pull/7/head`)의 조상이라 남고, f17c964 는 참조가 없지만 GitHub 이 아직 지우지 않았다, 지우려면 GitHub Support 에 요청하거나 레포를 다시 만들어야 한다(사용자 판단 대기)
- GitHub 이슈 1, 2 의 댓글은 옛 merge hash(f9b044d, ecdc12c)를 말한다, 새 hash 는 아래 짝을 본다, 댓글은 고치지 않았다
- 이 레포 기록(`project_manage/`) 안의 옛 7자리 hash 는 아래 짝으로 모두 새 hash 로 바꿨다
- 옛 hash -> 새 hash
  - 454e332 -> da95165 docs: 밤샘 실행 마무리 확인을 log 에 적음
  - 5625578 -> 58552f4 docs: 밤샘 실행의 아침 보고와 마지막 상태를 board 와 log 에 적음
  - 1dd84f0 -> 0d93e7c feat: 라벨 없는 FL 모의실험 코드와 작은 규모 실행 결과를 추가
  - bfc2119 -> 0d9fe1b docs: 시작 모델 오류율 이슈를 merge 하고 끝내며 기록을 마감함
  - ecdc12c -> 7d069d4 Merge pull request #7 from guny524/guny524/2-start-model-cer
  - 3135ab8 -> 398e2fb fix: 측정 코드의 준비 잠금, 설정 원자적 쓰기, 표본 목록 대조를 검토대로 고침
  - a410745 -> 87db078 docs: 앞 이슈 merge, worker 이슈 기록, 두 번째 검토 결과를 board 와 log 에 적음
  - f9b044d -> 8f7e917 Merge pull request #6 from guny524/guny524/1-app-skeleton-ci
  - 5a4c397 -> 6d642da fix: 측정 코드의 cache 삭제 범위, 판 고정, 재개와 LoRA 판정을 검토대로 고침
  - dc22e89 -> e763890 feat: 받아쓰기 interface, 전사 조각 타입, 가짜 엔진, 실시간 배수 계산을 추가
  - aac2755 -> 2337513 fix: Android debug 서명 키를 레포에 두어 모든 실행에서 같게 함
  - 75aeb5e -> c2c9e91 fix: Android 서명 키를 레포에서 가져오게 하고 README 의 빌드 설명을 맞춤
  - b0ce9cd -> 0ef6953 docs: 앱 뼈대 CI 재확인, 두 번째 검토, 시작 모델 검토 지적을 board 와 log 에 적음
  - 166d736 -> 269b2b6 docs: 두 이슈의 보고 확인과 PR, 검토 시작을 board 와 log 에 적음
  - d8b92ee -> 65bb8c4 feat: 시작 모델 후보의 오류율 측정 코드와 결과 문서를 추가
  - 12bb603 -> 4c4d2e4 fix: 설치 도구 해시 확인, action SHA 고정, Android 키 고정, IPC 시험 추가
  - f02469a -> 5cfca26 docs: 앱 뼈대 검토 결과와 고칠 칸 네 개를 board 와 log 에 적음
  - e591599 -> 8fee8ac docs: 앱 뼈대 CI 통과와 Codex 검토 시작을 board 와 log 에 적음
  - 7d68d19 -> 92b3043 docs: 앱 뼈대 보고 확인, 커밋 push, CI 시작을 board 와 log 에 적음
  - 91398a2 -> e6ea2be docs: 앱 뼈대 보고 확인과 CI 시작을 기록에 적음
  - a3ad95f -> 24ff6d2 feat: Tauri 2 앱 뼈대와 다섯 플랫폼 설치 파일 CI 를 추가
  - f17c964 -> 3169495 docs: 나머지 이슈 셋의 기록과 밤샘 실행 절차를 적음
  - 98ccde9 -> 158c9d3 chore: 프로젝트 기록, 라이선스, README 를 처음 넣음

## 2026-11-20 02:01 두 단계 전사 검수 밤샘 결과
- 사용자 요구대로 맞음·틀림 검수와 교정을 분리하고, 현재 버전에서 사용자가 확인한 음성 구간만 로컬 학습 대기열에 들어가도록 구현했다. 별도 안내에는 현재 로컬 처리와 앞으로 적용할 노이즈 및 보안 집계를 적었다
- 작업 브랜치 `guny524/5-verified-transcript`에 기능(ee20f81)과 Whisper macOS 최소 버전 수정(14b610c)을 올렸다. CI와 5개 플랫폼 설치 파일이 모두 통과했다. 세부 검토와 시험은 녹음·받아쓰기·수정 화면 이슈 기록에 있다
- 남아 있는 제품 작업은 마이크 입력, 실기기 확인, 실제 로컬 학습 엔진과 연합학습 전송이다. PyTorch 연구로는 사람 확인 데이터 한 건의 가중치 최적화까지 확인했다

## 2026-11-22 14:36 GitHub 이슈 체크리스트 정리
- 사용자 지시 원문: “github 에 이슈들이 $conversation-rag 예전에 managment-todo 처럼 해야할 내역이 checkbox 로 잘 list up 되어있는 상태들이어야해”
- 열린 GitHub 이슈 네 개와 각 이슈 board에 작업 순서, 코드 위치, 완료 판정과 남길 증거를 적었다. 게시 후 GitHub에서 본문을 다시 받아 네 이슈 모두 작성한 내용과 일치하는지 확인했다
- 첫 녹음과 파일 가져오기, 모델 선택, 전사 처리 중과 실패 복구, 첫 결과에서 검수·교정·내보내기까지를 녹음·받아쓰기·수정 화면 이슈에 붙였다. 공개 기준 커밋의 CI와 이후 로컬 화면 작업을 구별했다
- 남은 인계 결정: 최신 로컬 앱 소스를 기존 작업 브랜치에 올려 초안 PR을 만들지 사용자 확인 대기

## 2026-11-22 20:29 전사 화면 인계용 초안 PR
- 사용자 승인 원문: “네, 올리고 초안 PR 만들기”
- 기존 작업 브랜치 `guny524/5-verified-transcript`의 `73d85d7`을 인계 기준으로 올리고 [녹음과 기기 안 받아쓰기, 빠른 검수·교정 화면 초안 PR (#10)](https://github.com/guny524/seamone/pull/10)을 열었다. 최신 main을 합치며 README 충돌을 해결했다
- `make lint`, `make test`, Chrome 화면 시험 18개와 macOS 실제 Whisper 전사 시험을 확인했다. 패키지 빌드와 실제 설치 파일 조작은 [녹음·받아쓰기·수정 화면 이슈 (#5)](https://github.com/guny524/seamone/issues/5)에 남겼다

## 2026-11-22 21:55 작업분 develop 병합과 Orca 정리
- 당시 `main`에서 `develop`을 만들고 [녹음과 기기 안 받아쓰기, 빠른 검수·교정 화면 PR (#10)](https://github.com/guny524/seamone/pull/10)을 병합했다(`3ba1047`). 다섯 플랫폼 빌드를 통과한 PR의 앱 코드와 병합 결과가 같다
- 위 병합 커밋에서 lint, Rust·연구 회귀 시험, 데스크톱 엔진 시험과 Chrome 화면 시험이 통과했다. [develop 병합 커밋 CI](https://github.com/guny524/seamone/actions/runs/45024381990)
- 로컬 기준 작업 사본과 Orca의 새 worktree 기준을 `develop`으로 맞췄다. 병합된 녹음·교정과 엔진 interface 작업 사본, 종료한 탭 3개, 병합된 로컬 작업 브랜치 4개를 정리했다
- 병합 전인 연합학습 모의실험의 작업 사본은 보존했다. Orca 표시 이름을 `4-unlabeled-fl-sim`으로 바꾸고 [연합학습 모의실험 이슈 (#4)](https://github.com/guny524/seamone/issues/4)와 [소규모 실험 초안 PR (#9)](https://github.com/guny524/seamone/pull/9)을 연결했다

## 2026-11-24 01:12 야간 점검 시작
- 01:10:12 KST에 Orca에서 현재 메인 세션을 깨우는 한 시간 간격 타이머를 시작했다. 종료 시각은 08:00 KST다. 짧은 시험에서 메인 세션이 작업 중이면 입력을 건너뛰는 것을 확인했다
- `develop`의 `1cf5794` 기준 첫 전사 흐름 구현과 `0d93e7c` 기준 연합학습 모의실험 검토를 Codex Sol high 작업자 두 명에게 맡겼다
- 사용자가 먼저 시작한 두 명은 일을 마치고 이후에는 작업자를 한 명으로 줄이라고 지시했다. 첫 작업이 끝나면 자리를 다시 채우지 않는다

## 2026-11-24 02:12 첫 점검과 소규모 모의실험 검토
- 시간 점검 신호가 현재 메인 세션의 새 turn을 시작했다. 기존 작업자 두 명의 완료를 각각 확인했다
- 연합학습 모의실험 `0d93e7c`에서 연구 시험 129개가 통과했다. 저장 결과를 같은 발화로 재채점했고, 한 설정에서 Zeroth-Korean 문자 오류율은 8.36%에서 7.70%, FLEURS 한국어는 7.49%에서 7.94%로 바뀌었다
- 평가 자료 준비, 교사 모델 인자 전달과 결과 보존의 문제를 코드에서 확인했다. [연합학습 모의실험 이슈 (#4)](https://github.com/guny524/seamone/issues/4)에 남기고 게시 내용을 대조했다. 새 학습과 실제 기기 성능 측정은 이루어지지 않았다

## 2026-11-24 02:15 첫 전사 화면 검증과 후속 수정
- `1cf5794` 위의 미커밋 앱 화면 변경에서 `make lint`, Rust 시험 83개, Chrome 화면 시험 26개와 실제 Whisper small로 받아쓰는 개발 통합 시험을 메인이 직접 다시 통과시켰다. 실제 설치 파일에서 마이크를 쓰는 시험은 남았다
- Claude Opus medium 최종 검토(세션 `f8a4e3fd-5de4-439e-985e-6f555554c243`)에서 저장 안내가 작업 결과에 덮이는 문제와 보관함 조회 실패가 엔진 미지원으로 표시되는 문제를 확인했다. 검토자는 기본 브라우저가 없어 화면 시험을 실행하지 못했고, 메인은 설치된 Chrome을 지정해 확인했다
- [녹음·받아쓰기·수정 화면 이슈 (#5)](https://github.com/guny524/seamone/issues/5)에 수정과 회귀 시험 항목을 적고 게시본을 대조했다. 두 문제는 기존 앱 작업자 한 명이 같은 worktree에서 고친다. 모의실험 검토 작업 사본은 유지하고 끝난 작업자 탭만 정리했다

## 2026-11-24 02:30 첫 전사 후속 수정 검증
- `c4dbd0a` 위 미커밋 변경의 diff SHA-256은 `7fd7a15d1c9da843b52bd2ffabbd82286db0b4839b129ebe001f704030e1e27a`다. 저장 안내 유지, 보관함 첫 조회 실패와 재시도, 기존 목록 보존을 포함해 lint, Rust 시험 83개, Chrome 시험 32개를 다시 확인했다
- Claude Opus medium이 두 수정 사항을 확인했다(세션 `f8a4e3fd-5de4-439e-985e-6f555554c243`). 이 변경으로 생긴 다시 열기 안내 중복과 백그라운드 갱신 알림을 확인해 기존 앱 작업자 한 명에게 정리를 보냈다

## 2026-11-24 02:37 첫 전사 흐름 초안 PR
- 중복 열기 버튼과 백그라운드 로딩 알림에 대한 수정과 시험을 코드에서 대조했다. `develop`의 `39a5c2e`를 반영한 뒤 전체 diff의 SHA-256 `94386a1e72d2ec7b5e2ca56938452b5fc3eec1cdedf4c213c02e26f572cef1a0`에 대해 메인이 lint, Rust 시험 83개와 Chrome 시험 32개를 통과시켰다
- `7276a7b`을 [첫 전사 흐름 초안 PR (#11)](https://github.com/guny524/seamone/pull/11)에 올리고 [녹음·받아쓰기·수정 화면 이슈 (#5)](https://github.com/guny524/seamone/issues/5)의 체크리스트를 대조했다. 패키지 빌드와 실기기 시험은 남았다

## 2026-11-24 03:11 첫 전사 PR 빌드 확인
- `7276a7b`에서 PR CI와 macOS·Windows·Linux·Android 패키지, iOS 시뮬레이터 빌드가 모두 통과했다. 여섯 플랫폼별 아티팩트가 남아 있다. [PR CI](https://github.com/guny524/seamone/actions/runs/45229632089) · [기본 패키지](https://github.com/guny524/seamone/actions/runs/45229632146)
- 모델 선택과 실제 전사를 시험할 수 있게 같은 커밋의 기존 수동 빌드에서 `desktop-whisper`를 켰다. 기본 빌드 통과와 수동 빌드 진행 상태를 이슈와 PR 본문에 반영하고 대조했다. [Whisper 포함 패키지](https://github.com/guny524/seamone/actions/runs/45235655790)

## 2026-11-24 04:12 야간 작업 종료
- `7276a7b`의 [Whisper 포함 수동 빌드](https://github.com/guny524/seamone/actions/runs/45235655790)에서 여섯 job이 모두 통과했다. Apple Silicon DMG 체크섬이 맞았고 arm64 실행 파일, Whisper 함수 심볼과 최소 macOS 11.0 설정을 확인했다. DMG SHA-256은 `98fdfe6a29944f0352f622c6f48259793ec1991f74ae5477d6e9990d3e2f46f5`다
- 첫 전사 흐름 구현·자동 검증과 연합학습 모의실험 소규모 결과 검토를 마쳐 04:11:34 KST에 점검 프로세스와 Orca 타이머 탭을 종료했다. 확인용 DMG 사본은 지웠다
- 설치 파일을 직접 실행하는 마이크·모델 선택과 모바일 터치 확인은 [첫 전사 흐름 초안 PR (#11)](https://github.com/guny524/seamone/pull/11)에 남겼다. 병합 전인 앱과 모의실험 작업 사본은 후속 검증을 위해 보존했다

## 2026-11-25 18:20 컴파일과 CI에서 앱 구조와 검수 계약 검사

- 영상의 장면 전환을 색 히스토그램으로 찾고 4:20, 6:35, 6:45 재생 화면을 확인했다. 화면과 native가 공용 타입을 참조하는 예를 Seamone의 Rust crate와 IPC 명령에 적용했다. [영상의 의존 방향 그림](https://www.youtube.com/watch?v=nldpxmin-1M&t=405s) · [pstack의 코드 구조 개선 지침](https://github.com/cursor/plugins/blob/41e400932eca14f289e7db717a70455256b6d388/pstack/skills/correct/SKILL.md)
- `878e9c5`로 의존 경계, IPC 명령, 검수 동작의 증거 조건, 학습 입력 타입과 lint 예외 검사를 `develop`에 병합했다. 정상 코드와 여섯 가지 위반의 거절 원인을 확인했다. 외부망을 끊은 Linux에서 연구 시험 104개가 통과했고 Chrome에서는 다른 출처로 보낸 요청이 수신 서버에 도달하지 않았다. [구조 검사 PR (#12)](https://github.com/guny524/seamone/pull/12) · [거절 진단](https://github.com/guny524/seamone/actions/runs/45583994420)
- Windows 패키지 작업이 Cargo의 UTF-8 출력을 시스템 문자 인코딩으로 읽던 문제를 수정했다. 실제 빌드 결과와 남은 설치 시험은 [앱 뼈대 이슈의 컴파일·패키지 검사](0001-2026_11_19-app_skeleton_ci/issue_board.md#7-18-컴파일과-패키지-단계의-의존-경계)에 있다
- `3d82dec`에서 첫 전사의 보관함 갱신과 중복 요청 처리를 유지하며 새 IPC 명령을 연결했다. 기본 Rust 시험 88개(문서 시험 포함)와 Chrome 화면 시험 33개가 통과했다. 별도 기기에서 마이크로 녹음하고 모델을 선택해 전사하는 시험을 위해 PR은 초안으로 두었다. [첫 전사 초안 PR (#11)](https://github.com/guny524/seamone/pull/11) · [통합 커밋 CI](https://github.com/guny524/seamone/actions/runs/45582919461)
- 첫 전사 초안의 GitHub 비교 범위를 `develop`에 맞췄다. 현재 커밋 `1cb09ec`와 검증 커밋 `3d82dec`는 Git 트리 `4f04a72`가 같다. GitHub의 PR 변경 목록에서도 첫 전사 파일 여섯 개를 확인했다

## 2026-11-27 01:55 구조 검사 밤샘 실행의 첫 결과
- 사용자 지시로 pstack 개념을 레포에 적용하는 밤샘 실행을 시작했다(이슈 #15, 지시 원문은 이슈 board 1절, 운영 방침은 이 board 3절)
- worker 조사와 Opus 5.5 high 설계 검토를 거쳐 이번 밤 범위를 native 작업 crate 분리와 그 검사로 정했다(이슈 decision D1, D2)
- `b2d2990`, `ce55aac` 를 draft [PR #16](https://github.com/guny524/seamone/pull/16) 에 올렸다, 메인이 `make check-architecture`, `make lint`, `make test`(99개) 통과를 확인했다, 자세한 근거는 이슈 log 2026-11-27 01:50

## 2026-11-27 02:25 구조 검사 PR 의 iOS 빌드 실패
- draft PR #16 의 build run 45862643316 에서 iOS 시뮬레이터만 실패했다(`app/src-tauri/src/commands.rs:126` E0308), 나머지 다섯 산출물과 ci 두 run 은 통과했다, 고치는 일과 이 실수를 lint 단계에서 막는 모바일 컴파일 검사를 worker 가 하고 있다, 자세한 것은 이슈 log 2026-11-27 02:21

## 2026-11-27 02:26 Pi local LLM 실수 유도 시험 1
- 재생 확인 없이 모두 "맞음" 으로 만드는 command 를 SQL 로 바로 넣으라는 일에서, Pi 는 host 의 직접 SQL 은 검사 때문에 버렸지만 runtime 안에 증거 없이 저장하는 공개 함수를 더했고 모든 검사가 통과했다, 재생 증거를 타입으로 강제하는 다음 단계를 정했다, 자세한 것은 이슈 log 2026-11-27 02:24

## 2026-11-27 02:42 구조 검사 PR 갱신과 develop 기준 목록
- develop 에 lint 예외 기준 목록 `e13746d` 를 넣고 PR #16 에 `360a089` 를 올렸다, 메인이 시험 106개 통과를 확인했다, 자세한 것은 이슈 log 2026-11-27 02:40

## 2026-11-27 03:02 재생 증거 타입의 구멍
- Opus 검토가 5단계의 재생 증거 타입을 runtime 안 네 줄로 우회할 수 있음을 찾았다, 7단계로 고친다, 자세한 것은 이슈 log 2026-11-27 03:00

## 2026-11-27 03:25 PR 빌드 통과와 Pi 의 두 번째 우회
- PR #16 의 `e0b68d3` 이 CI 와 다섯 플랫폼 패키지 빌드를 통과했다, research 직접 삭제 lint `4c702fe` 를 더했다, Pi 재시험 2 는 Store 안의 SQL 로 확인 전사를 직접 쓰고 공개 API 목록을 스스로 고쳐 다시 우회했다, 자세한 것은 이슈 log 2026-11-27 03:25

## 2026-11-27 03:33 Pi 실수 유도 시험 2 결과
- 재생 증거 타입이 생긴 뒤에도 Pi 는 Store 안 SQL 로 확인 전사를 직접 만들고 공개 API 목록을 스스로 고쳐 모든 검사를 통과했다, 8단계로 core 전이 하나로 묶기와 목록의 base 대조를 정했다, 자세한 것은 이슈 log 2026-11-27 03:32

## 2026-11-27 03:40 재생 증거 발급 경로 고정
- 7단계 `d0a3d95` 를 PR #16 에 올렸다, 메인이 시험 119개 통과를 확인했다, 8단계를 시작했다, 자세한 것은 이슈 log 2026-11-27 03:40

## 2026-11-27 03:58 확인 전사 쓰기 막기와 기준 목록
- 8단계를 PR #16 에 올리고 기준 목록 네 개를 develop 에 넣었다(`fdf16ac`), Pi 재시험 2 의 우회가 세 곳에서 거절된다, 자세한 것은 이슈 log 2026-11-27 03:58

## 2026-11-27 04:03 PR #16 CI 실패와 merge 계획
- PR #16 CI 가 CI Python 의 오래된 SQLite 때문에 실패했다, 고친 뒤 merge 하고 남은 우회 9건은 새 PR 에서 고친다(이슈 decision D3), 자세한 것은 이슈 log 2026-11-27 04:03

## 2026-11-27 04:20 PR #16 merge
- runtime crate 분리와 구조 검사 PR #16 을 develop 에 merge 했다(`9ed37ed`), 다섯 플랫폼 패키지 빌드 통과, 열린 PR #11, #13, #14 에 develop 을 merge 해 push 했고 #13 의 base 를 develop 으로 바꿨다, 자세한 것은 이슈 log 2026-11-27 04:20

## 2026-11-27 04:36 열린 PR 옮기기 결과
- PR #11 과 #14 는 새 develop 위에서 CI 와 다섯 플랫폼 빌드를 통과했다, PR #13 은 main 과 비교한 옛 실행이 실패해 다시 돌린다, 자세한 것은 이슈 log 2026-11-27 04:36

## 2026-11-27 04:45 Pi 실수 유도 시험 3 결과
- Pi 는 증거 module 안에 새 함수를 만들고 기준 목록을 늘린 뒤 자기 커밋을 base 로 lint 를 통과시켰지만, 실제 develop 기준에서는 목록 대조가 "사람이 확인한 뒤에만 늘립니다" 로 거절했다, 기계 검사가 사람 확인 관문까지 넘기게 됐다, 자세한 것은 이슈 log 2026-11-27 04:45

## 2026-11-27 04:55 검사 자체 지키기 PR #18
- 같은 PR 의 검사 파일 수정으로 검사가 꺼지지 않게 하고 남은 우회를 실행 중 권한과 범위 검사로 막는 PR #18 을 올렸다, 사람이 확인한 뒤 merge 한다, 자세한 것은 이슈 log 2026-11-27 04:55

## 2026-11-27 05:02 PR #18 검토
- Opus 가 PR #18 에서 남은 우회와 정상 개발을 막는 과잉 검사를 찾아 10단계로 고친다, 자세한 것은 이슈 log 2026-11-27 05:02

## 2026-11-27 05:42 PR #18 갱신
- 쓰기 권한이 고정 SQL 을 직접 갖게 하고 과잉 검사를 줄인 `35f64ff` 를 PR #18 에 올렸다, 시험 131개 통과, 자세한 것은 이슈 log 2026-11-27 05:42

## 2026-11-27 05:46 Pi 실수 유도 시험 4 결과
- Pi 는 증거 module 안에 새 승인 종류를 만들어 검수 정책을 바꾸고 목록을 늘렸다, 실제 base 대조가 거절해 사람 확인으로 넘긴다, 네 번의 시험에서 기계 검사가 우회를 사람 확인 관문까지 몰았다, 자세한 것은 이슈 log 2026-11-27 05:46

## 2026-11-27 05:52 PR #18 마지막 검토
- Opus 마지막 검토가 회귀 하나와 과잉 검사, 남은 우회를 찾았다, 11단계로 회귀와 과잉부터 고치고 남은 우회를 어디까지 막을지는 사람에게 묻는다, 자세한 것은 이슈 log 2026-11-27 05:52

## 2026-11-27 06:20 아침 보고(구조 검사 밤샘 실행)
- 무엇을 했나: 사용자 지시로 pstack 개념을 레포에 적용했다, 저장소, 음성, 녹음, 받아쓰기, 학습 대기열을 `crates/seamone-runtime` 으로 옮기고(폴더 구조), host 의 직접 접근, 재생 증거 없는 확인 전사, research 의 공유 cache 삭제, 새 lint 억제, iOS 전용 코드 오류를 컴파일, lint, CI 에서 거르게 했다, Pi local LLM 으로 같은 함정을 네 번 시험해 우회가 나올 때마다 막았다
- merge 된 것: PR #16 "기기 안 저장소와 음성 처리를 runtime crate 로 옮기고 host 의 직접 접근을 검사로 막는다"(`9ed37ed`, 다섯 플랫폼 패키지 빌드 통과)
- develop 에 직접 넣은 것: lint 예외 기준 목록 `e13746d`, runtime 공개 API, 재생 증거 선언, 확인 전사 쓰기 함수, unsafe 파일 기준 목록 `fdf16ac`, 기록 커밋들
- 사람 확인을 기다리는 것: PR #18 "검사가 같은 PR 의 검사 파일 수정에 꺼지지 않게 하고 남은 우회를 실행 중 권한과 범위 검사로 막는다"(`34c110d`, `35f64ff`, `226d0a2`), 검사 파일을 고치므로 새 관문 job 이 이 PR 에서 실패하는 것이 맞다
- 옮긴 것: 열린 PR #11, #13, #14 에 develop 을 merge 해 새 구조 위에서 CI 와 패키지 빌드 통과(셋 다 draft, #13 base 를 develop 으로 바꿈)
- [밤사이 결정]: 이번 밤 범위(이슈 decision D1), 기존 lint 예외는 새로 더하는 것만 거절(D2), PR #16 먼저 merge 와 남은 우회는 새 PR(D3), PR #13 base 를 develop 으로
- 쓴 모델: worker Codex `gpt-6.1-sol` high 한 세션(바꾸지 않음), 검토 Claude Opus 5.5 high 다섯 번, 적용 확인 Pi `d0xin-swift-1.5-qwen3.8-27b-uncensored-fp8` 네 번
- 자세한 것은 이슈 log `0015-2026_11_27-pstack_structure_checks/issue_log.md`

## 2026-11-27 13:10 사용자 답과 PR #18 merge
- 사용자가 확인자를 메인에게 맡기고(D4) lint 예외는 설계로 없애고 주석은 pstack 대로 정리하며(D5) 재생 확인을 근본부터 고치라고(D6) 답했다, PR #18 을 merge 했고 develop 에 branch protection 을 걸었다, merge 뒤 드러난 iOS lint 실패를 12단계로 고친다, 자세한 것은 이슈 log 2026-11-27 13:10

## 2026-11-27 13:40 PR #21
- lint 예외를 설계로 모두 없애고 주석을 외부 사정만 남긴 PR #21 을 올렸다, 재생 확인 근본 해결 PR 1 을 시작했다, 자세한 것은 이슈 log 2026-11-27 13:40

## 2026-11-27 14:15 PR #21 고침
- PR #21 의 검토 지적 10건과 덤 1건을 고친 `16527b8` 을 올렸다, f32 축소를 f64 계산으로 없앴고 결과 차이는 샘플당 최대 1 LSB, 자세한 것은 이슈 log 2026-11-27 14:15

## 2026-11-27 14:22 PR #21 재검토
- 재검토가 남은 다섯 건을 찾아 두 번째 worker 에게 맡겼다, f32 축소를 없앤 결정은 이슈 decision D8, 자세한 것은 이슈 log 2026-11-27 14:22

## 2026-11-27 14:30 재생 확인 PR 1
- runtime 이 음성을 장치로 내보내고 끝까지 출력된 범위로만 증거를 만드는 draft PR #22 를 올렸다, `playback_ended` 를 없앴다, 자세한 것은 이슈 log 2026-11-27 14:30

## 2026-11-27 14:35 PR #21 고침 마무리
- PR #21 재검토의 남은 다섯 건을 고쳐 `87e602f` 로 올렸다, CI 확인 뒤 merge 한다, 자세한 것은 이슈 log 2026-11-27 14:35

## 2026-11-27 14:38 PR #22 검토
- PR #22 검토가 출력 장치 지연을 판정에 넣지 않는 문제 등 네 건을 찾았다, 지연을 읽어 더하기로 정하고 새 worker 에게 맡겼다, 자세한 것은 이슈 log 2026-11-27 14:38

## 2026-11-27 14:45 PR #21 merge 와 PR #23
- lint 예외 0개와 주석 정리 PR #21 을 merge 했다(`59c3cc2`), 빠른 검수를 runtime 출력으로 옮긴 draft PR #23 을 올렸다, 자세한 것은 이슈 log 2026-11-27 14:45

## 2026-11-27 14:44 develop CI 초록
- PR #21 merge 뒤 develop 최신 커밋 `0409cfe` 의 ci 가 모두 통과했다, PR #18 merge 뒤 드러난 iOS lint 실패가 풀렸다, 자세한 것은 이슈 log 2026-11-27 14:45 절 끝

## 2026-11-27 14:48 PR #23 검토
- PR #23 검토가 준비 중 Space 가 사라지는 회귀와 웹뷰 audio 검사의 빈틈 등 다섯 건을 찾아 worker 에게 맡겼다, 자세한 것은 이슈 log 2026-11-27 14:48

## 2026-11-27 15:00 PR #22 고침
- 재생 판정에 출력 장치 지연을 더한 고침과 develop merge 를 PR #22 에 올렸다, 자세한 것은 이슈 log 2026-11-27 15:00

## 2026-11-27 15:05 PR #23 고침
- PR #23 검토 다섯 건을 고쳐 `4882111` 로 올렸다, PR #22 위로 맞추는 merge 를 worker 가 한다, 자세한 것은 이슈 log 2026-11-27 15:05

## 2026-11-27 15:10 PR #22 재검토
- PR #22 고침의 재검토가 남은 세 건을 찾아 worker 에게 맡겼다, 실기기 재생 시험 절차를 이슈 board 에 사람 몫으로 적었다, 자세한 것은 이슈 log 2026-11-27 15:10

## 2026-11-27 15:15 PR #23 을 PR #22 위로
- PR #23 에 PR #22 의 새 고침과 develop 을 merge 했다(`e97596d`), 시험 170개 통과, 자세한 것은 이슈 log 2026-11-27 15:15

## 2026-11-27 15:23 PR #22 고침 마무리
- PR #22 재검토의 남은 세 건을 고쳐 `d3fa235` 로 올렸다, CI 확인 뒤 merge 한다, 자세한 것은 이슈 log 2026-11-27 15:23

## 2026-11-27 15:33 PR #22 merge
- runtime 이 음성을 장치로 내보내고 끝까지 출력된 범위로만 재생 증거를 만드는 PR #22 를 merge 했다(`9359b88`), 빠른 검수 PR #23 을 새 develop 위로 맞췄다, 자세한 것은 이슈 log 2026-11-27 15:33

## 2026-11-27 15:47 관문 workflow 와 default branch
- 검사 파일을 고친 PR 을 사람이 보게 하는 `pull_request_target` 관문이 default branch(main)에 파일이 없어 한 번도 돌지 않은 것을 찾고 default branch 를 develop 으로 바꿨다(이슈 decision D9), 자세한 것은 이슈 log 2026-11-27 15:47

## 2026-11-27 15:57 PR #23 Linux 빌드 고침
- make 를 부르는 모든 workflow job 에 Python 판을 고정하고 빠지면 구성 시험이 실패하게 한 `2d40b14` 를 PR #23 에 올렸다, 자세한 것은 이슈 log 2026-11-27 15:57
