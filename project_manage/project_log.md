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
