# 이슈 log
(형식은 `project-issue-conductor` skill 의 `references/record-format.md` 7절, 시각 순으로 쌓고 지우지 않는다)

## 2026-11-19 08:47 worker(Sonnet 5.5, effort 미확인) 보고
- 결과 한 줄: 라벨 없는 FL 모의실험이 작은 규모(기기 8명, 기기마다 발화 6개, 라운드 2, 시험 100개)로 끝까지 돌았고, 효과는 이 규모로 판정하지 못했다(A0 과의 CER 차이가 모두 그 차이의 표준오차의 1.3배 이하)
- 결과 문서: `/Users/min-jo/orca/workspaces/seamone/2-start-model-cer/research/results/2026_11_19_unlabeled_fl_sim.md`(한계 1절, 결론과 추천 2절, 결과 4절, 판정 5절, 다음 단계 6절, 다시 돌리는 법 7절)
- 못 한 것: 큰 규모 실행(약 3시간, 추측), A4 와 Q3(메인 지시로 하지 않았다), 방법 섞기

## 1. 끝낸 칸 (목적: board 7절)
- 체크한 칸: 7-1 기기 데이터, 7-3 FL 로 합치기, 7-4 정답 만들기, 7-5 방법 실행, 7-6(건너뜀을 적었다), 7-7 결과 문서, 7-8 정리와 보고
- 체크하지 않은 칸 하나: 7-2 학습 한 걸음, 코드와 학습 시간은 끝났으나 검증 칸의 "손실이 열 걸음 동안 내려간다" 를 돌리지 않았다(라운드 평균 손실은 기기가 달라 비교가 되지 않는다), 남은 것이다
- 지시와 달리 한 곳은 칸의 결과 줄과 6절에 적었다: 규모를 줄였다(기기마다 발화 6개, 시험 40개와 40개와 20개), 라운드 0, 2, 4 대신 마지막 라운드 뒤 한 번만 쟀다

## 2. 결과 (결과 문서 4절)
- 학습률 1e-4(board 의 출발값): 세 방법 모두 A0 과 거의 같다(Zeroth-Korean 40개 A0 8.36%, A2 8.25%, A3 8.41%, A1 8.41%, FLEURS 한국어 40개 모두 7.49%, FLEURS 영어 20개 모두 2.59%), A0 과 가설 글이 똑같은 시험 발화가 100개 중 A2 99개, A3 99개, A1 96개라 LoRA 가 거의 움직이지 않았다
- 학습률 3e-3(걸음이 적어 30배로 올린 탐색 값): A2 는 Zeroth 7.70%(A0 대비 -0.66%p, 상대 -7.9%), FLEURS 한국어 7.94%(+0.45%p, 상대 +6.0%), 영어 2.54%, A3 는 Zeroth 8.25%, FLEURS 한국어 7.44%, 영어 2.59%, A1 은 Zeroth 8.25%, FLEURS 한국어 8.50%(+1.01%p, 상대 +13.4%), 영어 2.64%, 모든 차이가 표준오차의 1.3배 이하라 학습 때문인지 시험 표본 때문인지 가르지 못한다, 이 실행에서는 A0 과 가설 글이 똑같은 발화가 100개 중 A2 42개, A3 62개, A1 14개라 모델은 움직였다
- 이슈 board 3-4절의 판정: "효과 있음" 은 판정하지 못했다(학습률 3e-3 의 A2 는 Zeroth 를 상대 5% 넘게 낮췄지만 FLEURS 한국어가 상대 3% 넘게 나빠졌고 모두 표준오차 안이다), "위쪽만 학습으로 충분" 은 A4 를 하지 않아 판정하지 못했다
- A3 의 정답 품질: 큰 모델의 정답은 진짜 정답 대비 CER 1.45%(48개), 학생의 정답은 7.17%와 9.89%(라운드마다 24개), 믿음도 0.9 이상으로 고른 조각은 라운드마다 24개 중 5개와 3개(두 번째 실행 5개와 4개)뿐이었고, 이 표본에서는 믿음도가 높은 구간의 CER 이 낮았다(0.9 이상 9개의 발화별 CER 평균 3.2%, 0.8 이상 0.9 미만 36개 9.0%, 0.6 이상 0.8 미만 3개 25.0%, 6번 기기가 두 라운드에 모두 뽑혀 서로 다른 발화는 42개이고 표준오차 없이 평균만 견줬다, 확정하지 못한다)
- 비용: 업데이트 2.359MB(글 디코더 LoRA 589,824개, fp32), 발화 하나의 학습 0.64초에서 0.86초(A1 과 A2, 묶음 4, Apple GPU)

## 3. 검증 명령과 결과
- `make -C research lint`: 통과(08:42, ruff 검사와 형식 검사 36개 파일)
- `make -C research test`: 129개 통과(08:42, 이 이슈의 새 시험 28개 포함: `test_fedavg.py` 13, `test_pseudo_label.py` 6, `test_fl_data.py` 6, `test_fl_train.py` 3, 나머지는 시작 모델 이슈의 시험이다)
- `make -C research fl-sim`: 두 번 끝까지 돌았다(학습률 1e-4 는 08:04 에서 08:14, 학습률 3e-3 은 08:21 에서 08:30, 로그 `.tmp/research/fl-sim.log`, `.tmp/research/fl-sim-lr3e-3.log`)
- 메인이 다시 계산할 때: `make -C research score MODELS="whisper-small fl-a2 fl-a3 fl-a1" SETS="zeroth_ko fleurs_ko fleurs_en"`(08:34 에 돌려 결과 문서의 값과 같음을 확인했다, 예: 모델이 같이 가진 앞 40개에서 whisper-small 8.36%, fl-a2 8.25%, fl-a3 8.41%, fl-a1 8.41%)
  - 학습률 3e-3 의 결과는 `MODELS="whisper-small fl-a2-lr3e-3 fl-a3-lr3e-3 fl-a1-lr3e-3"` 로 센다(Zeroth-Korean 에서 whisper-small 8.36%, fl-a2-lr3e-3 7.70%, fl-a3-lr3e-3 8.25%, fl-a1-lr3e-3 8.25% 를 확인했다)
  - 가설 글: `/Users/min-jo/orca/workspaces/seamone/2-start-model-cer/.tmp/research/hyp/fl-<방법>/<세트>.jsonl`(한 줄에 id, ref, hyp, seconds), 라운드 기록 `.tmp/research/fl_sim/<방법>.jsonl`, 큰 모델의 정답 `.tmp/research/pseudo/a2-whisper-large-v3-turbo.json`
  - A0 은 #2 가 잰 `hyp/whisper-small` 의 앞 40개, 40개, 20개다(새로 돌리지 않았다)

## 4. 남은 것과 이유
- 큰 규모 실행: `make -C research fl-sim FL_DEVICES=12 FL_UTTS=60 FL_ROUNDS=4 FL_PER_ROUND=6 FL_METHODS="a2 a3 a1" FL_EVAL="zeroth_ko=200 fleurs_ko=200 fleurs_en=100"`, 시간 한도에 들지 않는다, 전체 약 3시간은 이번 속도로 어림한 추측이다(결과 문서 6절)
- A4 와 Q3, 방법 섞기(7-6): 메인의 지시와 시간 때문에 하지 않았다
- 기기 화자 12명: 이번 train 조각 하나에 화자가 16명이라 시험 화자를 빼고 12명이 되는지는 `make -C research fl-data FL_DEVICES=12 FL_UTTS=60` 로 확인한다(확인하지 못했다)

## 5. 판단거리와 추천
1. 큰 규모 실행을 언제 돌릴지(약 3시간, 노트북을 쓰는 다른 일과 겹치지 않는 때)
   - 추천: 돌린다, 이번 규모로는 방법 사이의 차이를 가를 수 없다, 큰 규모 전에 A3 의 믿음도 문턱을 믿음도 분포에 맞춰 정한다(결과 문서 6절 3번)
2. `.tmp/hf` 의 Qwen3-ASR-1.7B 가중치 4.4GB 를 지울지
   - 추천: 속도를 재는 이슈(받아쓰기 엔진 interface 와 기기별 속도)가 쓰지 않으면 지운다, 이 이슈는 큰 모델의 정답을 whisper-large-v3-turbo 로 달아 쓰지 않았다
3. 서버 검증 관문의 시험 세트 크기를 같은 발화 차이의 표준오차로 정하는 것을 설계에 넣을지
   - 추천: 넣는다, 관문은 같은 발화에서 업데이트 전과 후를 견주므로 필요한 크기는 그 차이의 표준오차가 정한다
   - 이번 실행에서 그 표준오차가 40개에서 0.1%p 에서 0.8%p 로 업데이트가 모델을 바꾼 정도에 따라 달랐고, "상대 3% 넘게 나쁘게 하지 않는다"(이 CER 에서 약 0.25%p)를 표준오차의 2배로 가르려면 약 100개에서 1,600개가 든다(제곱근 비례로 어림한 추측)

## 6. 규칙과 다르게 한 곳, 숨기지 않을 것
- 기기마다 발화 수를 지시한 12개 대신 6개로 줄였다, 근거를 board 4-2 에 먼저 적고 하라는 메인의 시작 지시(07:57, `start-1.md` 의 보고 확인 줄)의 순서를 지키지 못했다: 6개로 만든 것이 08:00 이고 결정 기록(D1)과 board 4-2 에 근거를 적은 것은 08:05 다
  - 08:00 의 결정 근거는 시간 한도 08:35 와 #2 의 whisper-large-v3-turbo 속도(실시간의 1.6배에서 3.1배)로 어림한 정답 달기 시간이었고, 08:04 의 실측(48개에 약 3분, 모델 불러오기 포함 실시간의 2.2배)이 그 어림 안이었다(D1 에 적은 "48개에 약 3분" 은 그 실측이라 결정 뒤에 안 값이다)
- 메인 지시의 방법 목록에 없는 두 번째 실행(학습률 3e-3)을 스스로 더했다(`issue_decision.md` D6): 첫 실행이 A0 과 같아 방법 사이를 견줄 수 없어서이고 컴퓨터 시간 약 9분을 썼다, 결과는 "탐색 값" 으로 표시했고 효과의 증거로 쓰지 않았다
- 시험(`make test`)을 모델이 올라간 동안 돌렸다(08:03 에서 08:04 는 큰 모델의 정답 달기와, 08:17 에서 08:18 은 시작 모델 이슈의 Qwen3-ASR-1.7B LoRA 재시험과, 08:22 에서 08:23 은 두 번째 실행과 겹쳤다): 시험은 모델을 올리지 않지만(가짜 로더를 쓴다) torch 와 transformers 를 불러와 메모리를 같이 썼다, 올린 모델은 한 번에 하나였다(큰 모델의 정답 달기를 끝내고 내린 뒤 학생을 올렸고, 학생 실행 사이의 LoRA 재시험은 대기열로 이어 돌렸다)
- 08:00 에 돌린 기기 데이터 만들기의 옛 코드가 공유 `hub/datasets--kresnik--zeroth_korean` 를 통째로 지웠다(시작 모델 이슈의 검토 지적 7-9 의 결함과 같은 것), 그 폴더에는 이 이슈가 받은 파일뿐이어서 잃은 것은 없고 지금은 전용 cache 로 고쳤다(`issue_decision.md` D7)
- 시작 모델 이슈의 검토 지적 여섯 칸(08:15 지시)을 먼저 고치느라 이 이슈의 작업이 08:10 에서 08:23 사이 멈췄고(고친 것은 보고 2 와 메인의 커밋 5a4c397 에 있다), 두 번째 검토의 새 지적 셋(08:40 지시)을 고치느라 08:38 에서 08:45 사이 다시 멈췄다(보고 3), 그 동안 컴퓨터에서는 첫 실행이 끝나고(08:14) LoRA 재시험(08:14 에서 08:20)과 두 번째 실행(08:21 부터)이 차례로 돌았다, 줄인 규모는 없다
- D5(처음 LoRA 가 0 이라 학습 전 학생이 whisper-small 과 같은 글을 낸다)는 직접 대조하지 않았다, 학습률 1e-4 에서 A2 와 A3 의 가설이 A0 과 100개 중 99개 같은 것이 간접 증거다

## 7. 자원
- `.tmp/research/` 1.3MB(가설 글 `hyp/` 796KB, `fl_sim/` 24KB, `pseudo/` 8KB, 나머지 로그와 도구), `.tmp/hf/seamone_fl` 12MB(기기 wav 48개), 받은 train 조각 약 500MB 는 만든 직후 지웠다(전용 cache `.tmp/hf/fl-raw`, 공유 `hub/` 는 건드리지 않았다)
- `.tmp/hf`(메인 레포) 7.0GB: whisper-small 926MB, whisper-large-v3-turbo 1.5GB, Qwen3-ASR-1.7B 4.4GB, `seamone_eval` 169MB, `seamone_fl` 12MB, `research/.venv` 1.3GB, 디스크 여유 19.99GiB(`df -k /Users/min-jo`, 08:33)
- 남긴 것과 지울 조건
  - `.tmp/research/hyp/fl-*` 와 `fl_sim/` 와 `pseudo/`: 메인이 수치를 다시 계산해 확인할 때까지 둔다
  - `.tmp/research/` 의 결과 문서를 다시 조립하는 `assemble_fl.sh` 와 `fl_report.py` 와 `fl-doc-*.md`, 시작 모델 이슈의 도구(`assemble.py`, `make_blocks.sh`, `adopt_*.py` 등)와 `lora-before-7-11/`: 메인이 결과를 확인하고 merge 한 뒤 worktree 와 함께 지운다
  - 가중치 whisper-small 과 whisper-large-v3-turbo 와 `seamone_fl`: 큰 규모 실행이 쓴다, 그 실행이 끝나면 메인이 지운다

## 8. 변경 범위
- 기준 커밋 5a4c397(메인이 시작 모델 이슈의 첫 검토 반영을 이미 커밋했다), stage 하지 않고 두었다
  - 이 이슈의 새 파일: `research/seamone_research/fedavg.py`, `fl_data.py`, `fl_sim.py`, `fl_train.py`, `pseudo_label.py`, `research/data/fl_client_ids.json`, `research/tests/test_fedavg.py`, `test_fl_data.py`, `test_fl_train.py`, `test_pseudo_label.py`, `research/results/2026_11_19_unlabeled_fl_sim.md`
  - 이 이슈가 고친 추적 파일: `research/Makefile`(`fmt`, `fl-data`, `fl-label`, `fl-sim` target 과 `FL_*` 변수), `research/README.md`(fl 관련 줄과 `fmt` 줄)
  - 시작 모델 이슈의 두 번째 검토 반영(보고 3, 그 이슈 몫): `research/seamone_research/data.py`, `measure.py`, `research/tests/test_data_cache.py`, `test_measure_config.py`, 새 파일 `research/tests/test_measure_resume.py`, `research/README.md` 의 측정 설정 줄 한 줄(이 파일에 두 이슈의 줄이 섞여 있다)
  - 라벨 없는 FL 모의실험의 `fl_data.py` 와 `fl_train.py` 는 시작 모델 이슈가 이미 커밋한 함수(`resolve_model_path`, `remove_inside`)를 부르므로 이 이슈의 커밋은 그 뒤에 한다
- 기록(worktree 밖): 이 이슈의 `issue_board.md` 4, 5, 7, 8절, `issue_decision.md` D1 에서 D7, 이 보고 `issue_log.md`
