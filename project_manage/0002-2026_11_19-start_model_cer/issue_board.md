# 시작 모델 후보의 한국어 오류율을 같은 잣대로 잰다
(이 파일: 기준 브랜치 main 의 `project_manage/0002-2026_11_19-start_model_cer/issue_board.md`, 같은 폴더에 `issue_decision.md`, `issue_log.md`)
- 이슈: https://github.com/guny524/seamone/issues/2, 유형: research(조사)
- 운영 상태(상태, 브랜치, worktree, worker, 세션, 선행과 후속): 프로젝트 `project_manage/project_board.md` 4절(이슈) #2

## 1. 사용자 지시 원문
- 2026-11-19 "모델은 굳이 오픈소스 모델을 쓸 필요 없지. 우리가 어차피 파인튜닝을 할 거니까 FL로. 근데 이제 처음에 시작할 모델을 뭘로 할지만 정하는 게 좀 필요해. 근데 영어나 뭐 최대한 다양한 언어나 됐으면 좋겠고, 특히 한국어가 잘 돼야 돼."
- 2026-11-19 "최대한 일단 할 수 있는 거에서 모델 크기가 컸으면 좋겠어. 아니면은 폰과 노트북 모델은 따로 가는 걸로 하자."
- 2026-11-19 "터치 뭐 되는데 그 볼륨 없으니까 ... 찌꺼기들 잘 지우라고 해놓은 거 있거든. 그거 잘 준수해서 용량 크게 안 남게 해."

## 2. 배경
### 2-1. 질문
- Q1. 후보 모델마다 한국어 글자 오류율(CER)은 얼마인가, 이 답으로 정하는 것: 계층마다의 시작 모델(프로젝트 PD9, #3 의 속도와 합친다)
- Q2. 후보 모델마다 영어 오류율은 얼마인가, 이 답으로 정하는 것: 여러 언어를 지원한다는 조건을 지키는지
- Q3. 후보를 PyTorch 에서 학습 가능한 형태로 불러와 작은 덧붙임 가중치(LoRA)로 역전파 한 번이 도는가, 걸리는 시간과 메모리는 얼마인가, 이 답으로 정하는 것: #4 모의실험에 쓸 모델과 기기 안 학습의 가능성
### 2-2. 가설
- Q1: Qwen3-ASR-1.7B 가 가장 낮고 Whisper large-v3-turbo 가 그다음이다, Whisper small 과 base 는 크게 높다, 이유: 모델 카드와 공개 비교표의 경향
- Q3: Whisper 계열은 transformers 로 바로 된다, Qwen3-ASR 은 전용 패키지라 확인이 필요하다
### 2-3. 이미 아는 것
- 공개 비교표의 FLEURS 한국어 CER: Whisper large-v3 4.89%, large-v3-turbo 5.24%(양자화 Q8, https://models.handy.computer/languages/ko), 시험 조건이 달라 그대로 쓰지 않고 다시 잰다
- Qwen3-ASR 은 Apache-2.0, 52개 언어와 방언, 1.7B 와 0.6B 두 크기(https://huggingface.co/Qwen/Qwen3-ASR-0.6B), Whisper 는 MIT
- 프로젝트 PD6(계층 둘), PD7(속도 기준), PD11(실험은 PyTorch), PD12(디스크)

## 3. 목표와 범위
### 3-1. 요구
- R1. 모든 후보의 CER 을 같은 음성 표본과 같은 글 정규화로 잰다, 출처 Q1, Q2
- R2. 측정을 make target 하나로 다시 돌릴 수 있다, 출처 rule `execute.md` 1절
- R3. CER 계산에 단위 시험이 있다, 출처 rule `change.md` 6절
- R4. 후보마다 LoRA 역전파 한 번의 성공 여부, 시간, 메모리를 적는다, 출처 Q3
- R5. 내려받은 가중치와 데이터는 `/Users/min-jo/Documents/Developments/seamone/.tmp/hf` 에만 두고, 다음 이슈(#4)에 쓰지 않는 것은 측정이 끝나는 대로 지운다, 출처 PD12
### 3-2. 범위
- 후보: `openai/whisper-base`, `openai/whisper-small`, `openai/whisper-large-v3-turbo`, `openai/whisper-large-v3`, `Qwen/Qwen3-ASR-0.6B`, `Qwen/Qwen3-ASR-1.7B`
- 시험 세트: FLEURS 한국어(`ko_kr`)와 영어(`en_us`)의 test, Zeroth-Korean 의 test, 세트마다 고정 seed 로 뽑은 표본
- `research/`(uv 프로젝트), `research/Makefile`, 결과 문서
### 3-3. 제외 범위
- 기기 속도 측정, 이유: #3 이 앱에 들어갈 Rust 엔진으로 잰다
- 학습 실험, 이유: #4
- 맨 위 `Makefile`, `.github/workflows/`, `Cargo.toml`, 이유: #1 이 만든다, 이 이슈는 `research/` 안만 고친다
### 3-4. 완료 기준
- 판정 기준: 여섯 후보의 한국어 두 세트와 영어 한 세트 CER 이 결과 문서에 있다, 재지 못한 후보는 무엇을 해 봤고 왜 실패했는지 적혀 있다
- 산출물: `research/results/2026_11_19_start_model_cer.md`(조건, 표본 수, 정규화 규칙, 결과, 한계, 추천), 표본 id 목록, `make -C research cer` 로 다시 돌릴 수 있는 코드
### 3-5. 검증 방법
- `make -C research lint`, `make -C research test`, `make -C research cer`
- 메인이 결과 문서의 수치 가운데 한 모델을 골라 저장된 가설 글로 CER 을 다시 계산해 대조한다
### 3-6. 의존성과 관련 링크
- 후속: #4 라벨 없는 FL 모의실험(이 이슈의 평가 코드, 데이터 준비, 남긴 가중치를 쓴다), #3 받아쓰기 엔진 속도(두 결과를 합쳐 PD9 를 정한다)
### 3-7. 확인 필요
- 사실: 노트북은 M1 16GB, 디스크 여유 49GB, 시스템 Python 은 3.15 라 torch 가 맞지 않을 수 있어 uv 로 Python 3.12 를 받는다, 근거 2026-11-19 03:50 조회
- 가정: FLEURS(`google/fleurs`)와 Zeroth-Korean(`kresnik/zeroth_korean`)을 Hugging Face 에서 로그인 없이 받을 수 있다, 틀리면 받을 수 있는 다른 공개 한국어 낭독 세트로 바꾸고 그 사실을 적는다
- 가정: Qwen3-ASR 이 Apple GPU(MPS)나 CPU 에서 돈다, 틀리면 해 본 것과 오류를 적고 그 후보는 "재지 못함" 으로 둔다
- 질문: 없음
### 3-8. 우선 참조할 파일
- `/Users/min-jo/Documents/Developments/seamone/project_manage/project_decision.md` PD6, PD7, PD9, PD11, PD12
- skill `project-structure`, `set-lint`, `design-test`

---
(아래는 AI 가 쓰는 절)

## 4. 조사
### 4-1. 조사 방법
- 시험 세트 셋(FLEURS 한국어와 영어의 test, Zeroth-Korean 의 test)에서 고정한 방법(30초 이하, 문장마다 녹음 하나, seed 와 id 의 SHA-256 순서, D4)으로 표본을 뽑아 `research/data/eval_ids.json` 에 두었다(한국어 FLEURS 200개, 영어 FLEURS 100개, Zeroth 200개, 음성 합계 5497.4초)
- 후보 여섯 모델을 같은 interface(`research/seamone_research/transcribe.py`: 언어 지정, greedy)로 풀어 써서 가설 글을 `.tmp/research/hyp/<모델>/<세트>.jsonl` 에 쌓고, 따로 `score.py` 와 `compare.py` 가 그 글로 CER(D5 의 정규화, 세트 전체의 편집 거리 합을 정답 글자 수 합으로 나눈 값)과 표준오차를 계산한다(D6)
- 조건 전부와 결과 전부는 `research/results/2026_11_19_start_model_cer.md`(아래 "결과 문서") 2절과 3절
- 환경: MacBook Air M1 16GB, Apple GPU(MPS) fp16, 묶음 크기 16(whisper-base, whisper-small)과 4(그 밖), `nice -n 10`(D8, D9), 측정하는 동안 다른 worker 의 Rust 빌드로 load average 25에서 65, swap 5GB 에서 15GB
- 표본을 줄였다(D10): whisper-large-v3-turbo 는 한국어 FLEURS 20개, 영어 12개, Zeroth 16개, Qwen3-ASR-0.6B 는 19개, 8개, 8개, Qwen3-ASR-1.7B 는 12개, 8개, 8개, whisper-base 와 whisper-small 은 전부, whisper-large-v3 는 재지 못했다
### 4-2. 질문별 찾은 사실
- 한국어 CER(Q1): 다섯 모델이 모두 푼 같은 발화 20개(FLEURS 12개와 Zeroth 8개)의 합산 CER 은 Qwen3-ASR-1.7B 2.09%, whisper-large-v3-turbo 2.93%, Qwen3-ASR-0.6B 3.03%, whisper-small 5.85%, whisper-base 10.97%(표준오차 ±1.2%p 에서 ±1.6%p), 출처 결과 문서 3-3절 `make -C research compare`, 정확도 중간(표본이 작다)
  - 같은 발화의 CER 차이(앞 모델에서 뒤 모델을 뺀 값): Qwen3-ASR-1.7B 와 large-v3-turbo -0.84%p(±0.39%p), Qwen3-ASR-1.7B 와 Qwen3-ASR-0.6B -0.94%p(±0.35%p), Qwen3-ASR-0.6B 와 large-v3-turbo +0.08%p(±0.66%p), large-v3-turbo 와 small -2.39%p(±0.88%p), 출처 결과 문서 3-3절
  - 모델마다 잰 전부(n 이 다르므로 서로 견주지 않는다): FLEURS 한국어는 base 13.66%(n=200), small 8.10%(200), large-v3-turbo 4.67%(20), Qwen3-ASR-0.6B 4.77%(19), Qwen3-ASR-1.7B 2.34%(12), Zeroth 는 base 16.86%(200), small 10.99%(200), large-v3-turbo 2.17%(16), Qwen3-ASR-0.6B 2.79%(8), Qwen3-ASR-1.7B 1.68%(8), 출처 결과 문서 3-1절
- 영어 CER(Q2): 같은 FLEURS 영어 8개에서 Qwen3-ASR-1.7B 2.37%(±1.12%p), whisper-small 과 large-v3-turbo 와 Qwen3-ASR-0.6B 3.15%, whisper-base 5.39%(±1.82%p), 모델마다 잰 전부는 base 5.23%(n=100), small 2.99%(100), large-v3-turbo 1.97%(12), Qwen3-ASR-0.6B 3.15%(8), Qwen3-ASR-1.7B 2.37%(8), 출처 결과 문서 3-1절과 3-2절, 정확도 낮음(8개)
- LoRA 역전파 한 번(Q3): TODO(whisper-small 의 Apple GPU 시험이 도는 중이다)
- 디스크 여유(`df -h /Users/min-jo` 의 Avail, 모델 하나가 끝날 때): whisper-base 22GiB(04:54), whisper-small 25GiB(05:13), whisper-large-v3-turbo 23GiB(05:36), Qwen3-ASR-1.7B 21GiB(05:45), Qwen3-ASR-0.6B 22GiB(05:48), LoRA 시험이 도는 중 15GiB(06:00), 가중치 지운 뒤 18GiB(06:03), 재개 전 값은 06:30 에 적는다
### 4-3. 확인하지 못한 것
- whisper-large-v3 의 오류율, 왜 못 했나: 가중치를 받아 두었으나 메인의 메모리와 디스크 부족 지시(05:30, 05:58)로 측정을 시작하지 못했고 가중치를 지웠다(D12), 확인하려면 디스크 여유 18GB 이상에서 `make -C research prefetch MODELS=whisper-large-v3` 와 `make -C research cer MODELS=whisper-large-v3 N="fleurs_ko=8 fleurs_en=4 zeroth_ko=4"`
- Qwen3-ASR-1.7B 와 whisper-large-v3-turbo 의 오류율 차이가 참인지, 왜: 같은 발화 20개에서 차이가 표준오차의 약 2.2배라 겨우 기운다, 확인하려면 한국어 60개씩과 영어 40개로 늘린다(메인이 06:30 뒤 재개를 지시했다)
- 회의 음성과 숫자가 섞인 말에서의 오류율, 왜: 시험 세트가 낭독 음성이고 정규화가 숫자를 바꾸지 않는다(결과 문서 5절), 확인하려면 회의 녹음과 숫자 정규화를 더한다
- 다른 언어의 오류율, 왜: 이 이슈는 한국어와 영어만 쟀다
- 속도, 왜: 속도는 받아쓰기 엔진 이슈가 Rust 엔진으로 잰다

## 5. 결론과 판정
### 5-1. 질문별 결론
- 한국어 오류율(Q1): 권고, 한국어 오류율이 낮은 순서 Qwen3-ASR-1.7B, whisper-large-v3-turbo 와 Qwen3-ASR-0.6B(구별되지 않는다), whisper-small(앞의 둘의 약 2배), whisper-base(훨씬 높다)를 계층마다 시작 모델을 고르는 근거로 쓴다, 근거 4-2 의 같은 발화 20개 합산과 같은 발화 쌍 차이, 정확도 중간(표본이 작아 Qwen3-ASR-1.7B 와 large-v3-turbo 의 차이는 표준오차의 약 2.2배로 확정이 아니다)
  - 2-2절의 가설과 비교: "Qwen3-ASR-1.7B 가 가장 낮다" 는 방향이 맞고 확정은 아니다, "whisper-large-v3-turbo 가 그다음" 은 Qwen3-ASR-0.6B 와 구별되지 않았다, "whisper-small 과 base 는 크게 높다" 는 맞다
- 영어 오류율(Q2): 판정, 잰 다섯 모델 모두 같은 영어 8개에서 CER 이 6% 아래(whisper-base 가 5.39% 로 가장 높다)라 영어가 막는 후보는 없다, 근거 4-2, 정확도 낮음(8개), whisper-large-v3 는 재지 못했다
- LoRA 역전파 한 번(Q3): TODO
### 5-9. 이 이슈의 결론
- 후보의 한국어 오류율 순서를 냈다(Qwen3-ASR-1.7B, large-v3-turbo 와 Qwen3-ASR-0.6B, small, base), 데스크톱 계층은 Qwen3-ASR-1.7B 를 첫 후보로 whisper-large-v3-turbo 를 대안으로, 폰 계층은 Qwen3-ASR-0.6B 와 whisper-small 을 후보로 두고, 시작 모델은 속도를 잰 뒤 메인이 정한다(프로젝트 결정 "시작 모델"), `issue_decision.md` D4 에서 D12, 근거 문서 `research/results/2026_11_19_start_model_cer.md`

## 6. 사용자 승인
- 해당 없음: 측정이고 새 사람 몫의 결정이 없다, 시작 모델을 고르는 일은 프로젝트 PD9 에서 한다

## 7. 할 일과 검증
(순서대로 한다, 시간 한도: 모델 하나의 측정이 25분을 넘을 것 같으면 표본을 줄이고 줄인 수를 적는다, 이 이슈 전체는 2시간 30분 안에 끝낸다)
(다른 worker 가 같은 노트북에서 Rust 를 빌드한다, 메모리가 16GB 라 모델은 한 번에 하나만 올린다)
- [x] 7-1. 환경: `research/` 에 uv 프로젝트(`pyproject.toml`, Python 3.12, torch, transformers, datasets, 오류율 계산 라이브러리, ruff, pytest)를 만든다, `research/Makefile` 에 `lint`(ruff), `test`(pytest), `cer`(측정) target 을 만들고 모든 target 이 `HF_HOME=/Users/min-jo/Documents/Developments/seamone/.tmp/hf` 를 쓰게 한다, 가상환경은 `research/.venv` / 검증: `make -C research lint`, `make -C research test` / 결과: `research/pyproject.toml:5-24`(Python 3.12, 의존성은 board 와 달리 datasets 를 뺐고 D1, `:31-38` 에서 qwen-asr 의 데모용 의존성을 뺐다 D2), `research/uv.lock`(판 번호는 이 파일이 원본), `research/Makefile:7-8`(HF_HOME 을 git 공통 폴더에서 찾은 메인 레포의 `.tmp/hf` 로 정한다 D7)와 target install, eval-ids, lint, test, prefetch, cer, score, compare, lora(`:27-68`), `research/.gitignore`, `research/README.md`, 가상환경 `research/.venv` 1.3GB, 검증: `make -C research lint` 통과(ruff 검사와 형식 검사), `make -C research test` 47개 통과(마지막 실행 06:03)
- [x] 7-2. CER 계산: `research/seamone_research/cer.py`, 정규화는 유니코드 NFC, 소문자, 문장부호와 공백 제거, 숫자는 바꾸지 않는다(정답은 "오" 이고 모델은 "5" 로 쓴 경우 오류로 잡히는 한계를 결과 문서에 적는다), CER 은 세트 전체의 편집 거리 합을 정답 글자 수 합으로 나눈다 / 검증: 단위 시험(같은 글 0, 공백과 문장부호만 다른 글 0, 한 글자 바뀜, 빈 가설, 한글과 영어 섞임) / 결과: `research/seamone_research/cer.py:26` normalize(NFC, 소문자, 범주 P*, Z*, Cc, Cf 제거, 숫자와 S* 는 둔다 D5), `:65` count_errors(jiwer 로 대체, 삭제, 삽입 수), `:84` corpus_counts(세트 전체 합), `:91` ratio_estimate 와 `:104` corpus_standard_error(표준오차), 검증: `research/tests/test_cer.py`(같은 글 0, 공백과 문장부호만 다른 글 0, 한 글자 바뀜, 빈 가설, 한글과 영어 섞임, 숫자 안 바꿈, 세트 합산 등 18개 함수), 시험이 결함을 잡는지는 scratchpad 사본에서 일곱 가지 변이(정답 글자 수를 안 더함, 문장부호를 안 지움, 기호까지 지움, NFC 안 함, 30초 안 거름, 문장마다 하나로 안 줄임, 공통 앞부분을 가장 긴 쪽으로 잡음)로 확인했고 하나는 시험을 고쳐 잡게 했다
- [x] 7-3. 시험 세트: `research/seamone_research/data.py`, FLEURS `ko_kr` test 200개, `en_us` test 100개, Zeroth-Korean test 200개를 seed 고정으로 뽑아 16kHz mono 로 맞춘다, 뽑은 id 목록을 `research/data/eval_ids.json` 으로 커밋하고 음성은 `.tmp/hf` 에만 둔다, 세트 전체를 받지 않고 test split 만 받는다 / 검증: id 목록의 수와 총 음성 길이를 출력해 적는다 / 결과: `research/seamone_research/data.py:93` select_ids(D4), `:164` build_eval_ids, `:260` prepare_audio(test split 파일만 받아 뽑힌 녹음을 16kHz mono wav 로 `.tmp/hf/seamone_eval/` 에 두고 원본은 지운다), `:283` load_eval_set, `research/data/eval_ids.json`(540줄), 검증: `make -C research eval-ids` 출력이 fleurs_ko 200개 2569.0초(후보 270문장), fleurs_en 100개 993.7초(후보 350문장), zeroth_ko 200개 1934.7초(후보 457개), 음성 파일 500개 합계 169MB
- [x] 7-4. 추론: `research/seamone_research/transcribe.py`, Whisper 계열은 transformers, Qwen3-ASR 은 공식 패키지로 같은 interface(음성 목록을 받아 글 목록을 낸다) 뒤에 둔다, 언어는 세트의 언어로 지정하고 greedy 로 푼다, 가설 글은 `.tmp/research/hyp/<모델>/<세트>.jsonl` 에 둔다 / 검증: 모델마다 음성 5개로 먼저 돌려 글이 나오는지 본다 / 결과: `research/seamone_research/transcribe.py:24` Transcriber(음성 목록과 언어를 받아 글 목록을 낸다), `:30` WhisperTranscriber(transformers), `:60` QwenAsrTranscriber(qwen-asr, 언어 이름 Korean 과 English), `research/seamone_research/models.py:11` 모델마다 device 와 dtype 과 묶음 크기, `research/seamone_research/measure.py:86` measure_set(가설 글을 eval_ids 순서로 jsonl 에 쌓고 이어 쓴다 D6), 검증: whisper-base 는 `N=5` 로, Qwen3-ASR-0.6B 는 3개로 먼저 돌려 글이 나오는 것을 봤고, 나머지 모델은 첫 묶음의 가설 글을 정답과 눈으로 대조했다(글이 정상이고 환각이 없었다)
- [ ] 7-5. 측정: 작은 모델부터(base, small, turbo, Qwen 0.6B, Qwen 1.7B, large-v3) 돌려 세트마다 CER 과 걸린 시간을 적는다, 한 모델이 끝날 때마다 `df -h` 의 여유를 적고, #4 에 쓰지 않는 가중치(base, large-v3, 그리고 Qwen 두 크기 가운데 오류율이 높은 쪽)는 그 자리에서 `.tmp/hf` 에서 지운다, 남기는 것은 whisper-small, large-v3-turbo, Qwen 가운데 가장 좋은 것 / 검증: `make -C research cer` 가 표본 5개짜리 빠른 모드로 끝까지 돈다
- [ ] 7-6. 학습 가능성: 남긴 모델마다 LoRA 를 붙여 음성 4개 한 묶음으로 순전파와 역전파 한 번을 돌리고 성공 여부, 걸린 시간, 메모리 최고치를 적는다(Apple GPU 와 CPU 둘 다) / 검증: 손실 값이 숫자로 나온다
- [ ] 7-7. 결과 문서 `research/results/2026_11_19_start_model_cer.md` 를 쓰고 이 board 4절과 5절을 채운다, 표를 쓰지 않고 목록으로 쓴다, 추천에는 데스크톱 계층과 폰 계층 후보를 나눠 적되 속도는 #3 에서 잰다고 밝힌다
- [ ] 7-8. 8절 수정 요약을 쓰고 보고를 쓴다, 보고에 지금 `.tmp/hf` 와 `research/.venv` 의 크기를 적는다

## 8. 수정 요약
- TODO

## 9. 지금 상태
- 메인이 직접 확인한 것: 없음
- worker 주장(아직 확인 전): 없음
- 남은 것: 7절 전부
- 판단거리: 없음

## 10. Closing
### 10-1. 검토
- TODO
### 10-2. 커밋 직전 증거 대조
- TODO
### 10-3. merge 직전 재확인
- TODO
### 10-4. 사람 시험
- 해당 없음: 측정 결과는 명령으로 확인한다
### 10-5. 자원
- TODO(`.tmp/hf` 에 남긴 것과 지운 것, `research/.venv`)
### 10-6. 이슈 마무리 코멘트
- 상태: TODO
### 10-7. 끝내기 승인
- TODO
### 10-8. 커밋과 merge
- TODO

## 11. 참고
- Qwen3-ASR 모델 카드 https://huggingface.co/Qwen/Qwen3-ASR-1.7B, 쓰는 법과 지원 언어
- FLEURS https://huggingface.co/datasets/google/fleurs, 여러 언어 낭독 음성(CC BY 4.0)
- Zeroth-Korean https://huggingface.co/datasets/kresnik/zeroth_korean, 한국어 낭독 음성(CC BY 4.0), 화자 id 가 있어 #4 에서 기기로 나눈다
