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
- Qwen3-ASR 은 Apache-2.0, 52개 언어와 방언, 1.7B 와 0.6B 두 크기(https://huggingface.co/Qwen/Qwen3-ASR-0.6B), Whisper 는 모델 카드 license 태그로 large-v3-turbo 만 MIT 이고 base, small, large-v3 는 Apache-2.0(2026-11-19 research worker 가 조회, 메인이 고침)
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
- 환경: MacBook Air M1 16GB, Apple GPU(MPS) fp16, 묶음 크기 16(whisper-base, whisper-small)과 4(그 밖), `nice -n 10`(D8, D9), 측정하는 동안 다른 worker 의 Rust 빌드로 load average 25에서 144, swap 5GB 에서 15GB
- 표본을 줄였다(D10, D14, D15): whisper-large-v3-turbo 는 한국어 FLEURS 56개, 영어 FLEURS 40개, Zeroth 16개, Qwen3-ASR-1.7B 는 56개, 40개, 8개, Qwen3-ASR-0.6B 는 19개, 8개, 8개, whisper-base 와 whisper-small 은 전부, whisper-large-v3 는 재지 못했다, Qwen3-ASR-1.7B 한국어는 06:59 에 부하와 디스크 여유 때문에 56개에서 멈췄다
### 4-2. 질문별 찾은 사실
- 한국어 CER(Q1): 네 모델이 모두 푼 같은 발화 64개(FLEURS 56개와 Zeroth 8개)의 합산 CER 은 Qwen3-ASR-1.7B 4.19%(±1.08%p), whisper-large-v3-turbo 5.44%(±1.20%p), whisper-small 7.47%(±1.21%p), whisper-base 13.89%(±1.67%p), 출처 결과 문서 3-3절 `make -C research compare`, 정확도 중간(표본이 작다)
  - 같은 발화의 CER 차이(앞 모델에서 뒤 모델을 뺀 값): Qwen3-ASR-1.7B 와 large-v3-turbo -1.25%p(±0.46%p, 표준오차의 약 2.7배, 64개), Qwen3-ASR-1.7B 와 small -3.28%p(±0.69%p), large-v3-turbo 와 small -2.30%p(±0.78%p, 72개), small 과 base -5.71%p(±0.34%p, 400개), 같은 한국어 27개에서 Qwen3-ASR-0.6B 와 large-v3-turbo +0.08%p(±0.66%p), Qwen3-ASR-1.7B 와 Qwen3-ASR-0.6B -1.23%p(±0.37%p), 출처 결과 문서 3-3절
  - 모델마다 잰 전부(n 이 다르므로 서로 견주지 않는다): FLEURS 한국어는 base 13.66%(n=200), small 8.10%(200), large-v3-turbo 5.83%(56), Qwen3-ASR-0.6B 4.77%(19), Qwen3-ASR-1.7B 4.53%(56), Zeroth 는 base 16.86%(200), small 10.99%(200), large-v3-turbo 2.17%(16), Qwen3-ASR-0.6B 2.79%(8), Qwen3-ASR-1.7B 1.68%(8), 출처 결과 문서 3-1절
- 영어 CER(Q2): 같은 FLEURS 영어 40개에서 Qwen3-ASR-1.7B 1.78%(±0.48%p), whisper-large-v3-turbo 2.42%(±0.52%p), whisper-small 3.68%(±0.63%p), whisper-base 5.30%(±0.87%p), Qwen3-ASR-1.7B 와 large-v3-turbo 의 차이 -0.64%p(±0.23%p, 약 2.8배), 모델마다 잰 전부는 base 5.23%(n=100), small 2.99%(100), large-v3-turbo 2.42%(40), Qwen3-ASR-0.6B 3.15%(8), Qwen3-ASR-1.7B 1.78%(40), 출처 결과 문서 3-1절과 3-3절, 정확도 중간(40개)
- LoRA 역전파 한 번(Q3): 글 디코더만 붙이는 범위는 Apple GPU(fp16)에서 whisper-small 묶음 4(두 번째 걸음 2.4초, GPU 메모리 2379MiB), whisper-large-v3-turbo 묶음 4(9.9초, 3147MiB), Qwen3-ASR-1.7B 묶음 1(4.4초, 6460MiB)이 성공했고(손실이 숫자, 기울기 모두 유한), Qwen3-ASR-1.7B 묶음 4 는 GPU 메모리 한도 11.84GiB 를 넘어 실패했다, 음성 인코더까지 붙이는 범위 all 은 whisper-small 이 묶음 1 에서 성공했고(3.6초, 3364MiB) 묶음 4 는 32분 넘게 끝나지 않아 06:20 에 멈췄다, CPU 는 whisper-small 묶음 4 가 범위 decoder 약 31초, 범위 all 약 210초(load average 45 에서 100)로 성공했다, 못 잰 것은 whisper-large-v3-turbo 와 Qwen3-ASR-1.7B 의 범위 all 과 두 모델의 CPU, 출처 결과 문서 4절, 정확도 중간
- 디스크 여유(`df -h /Users/min-jo` 의 Avail, 모델 하나가 끝날 때): whisper-base 22GiB(04:54), whisper-small 25GiB(05:13), whisper-large-v3-turbo 23GiB(05:36), Qwen3-ASR-1.7B 21GiB(05:45), Qwen3-ASR-0.6B 22GiB(05:48), LoRA 시험이 도는 중 15GiB(06:00), 가중치 지운 뒤 18GiB(06:03), 앞의 값은 `df -h` 의 표시값이라 실제보다 최대 1GiB 크다(06:22 에 `df -k` 로 17.02GiB 였는데 `df -h` 는 18Gi 로 보였다), 디스크 여유는 swap 에 따라 17GiB 에서 26GiB 사이를 오갔다(06:22 에서 06:33), 재개 전 06:32 에 `df -k` 로 23.69GiB(18GiB 이상이라 재개했다, 대기열 실행기가 상대 경로 실수로 06:30 에 뜨지 않아 2분 늦게 시작했다), 재개 뒤 가장 낮은 값 16.2GiB(06:58, Qwen3-ASR-1.7B 한국어 측정 중), 측정이 끝난 뒤 17.9GiB(07:09), `~/.cache/uv` 에서 쓰는 중이 아닌 archive 487개를 지운 뒤(07:21 전) 22.01GiB, 07:22 에 20.98GiB(`df -k`)
### 4-3. 확인하지 못한 것
- whisper-large-v3 의 오류율, 왜 못 했나: 가중치를 받아 두었으나 메인의 메모리와 디스크 부족 지시(05:30, 05:58)로 측정을 시작하지 못했고 가중치를 지웠다(D12), 확인하려면 디스크 여유 18GB 이상에서 `make -C research prefetch MODELS=whisper-large-v3` 와 `make -C research cer MODELS=whisper-large-v3 N="fleurs_ko=8 fleurs_en=4 zeroth_ko=4"`
- Qwen3-ASR-1.7B 와 whisper-large-v3-turbo 의 오류율 차이가 참인지, 왜: 한국어 64개에서 표준오차의 약 2.7배, 영어 40개에서 약 2.8배라 기울지만 확정이 아니다, Zeroth-Korean 은 Qwen3-ASR-1.7B 가 8개뿐이다, 확인하려면 부하가 낮을 때 Zeroth-Korean 을 60개로 늘린다(`make -C research cer MODELS=qwen3-asr-1.7b SETS=zeroth_ko N=60`)
- 회의 음성과 숫자가 섞인 말에서의 오류율, 왜: 시험 세트가 낭독 음성이고 정규화가 숫자를 바꾸지 않는다(결과 문서 5절), 확인하려면 회의 녹음과 숫자 정규화를 더한다
- 다른 언어의 오류율, 왜: 이 이슈는 한국어와 영어만 쟀다
- 속도, 왜: 속도는 받아쓰기 엔진 이슈가 Rust 엔진으로 잰다
- whisper-large-v3-turbo 와 Qwen3-ASR-1.7B 의 LoRA 범위 all 시험과 두 모델의 CPU LoRA 시험, 왜: 07:00 에 부하(load average 40 에서 140)와 디스크 여유 17GiB 안팎에서 부하를 키울 것 같아 대기열에서 뺐다, 확인하려면 `make -C research lora LORA_MODELS=whisper-large-v3-turbo LORA_DEVICES=mps LORA_BATCH=1 LORA_SCOPE=all` 와 `LORA_DEVICES=cpu LORA_SCOPE=decoder`

## 5. 결론과 판정
### 5-1. 질문별 결론
- 한국어 오류율(Q1): 권고, 한국어 오류율이 낮은 순서 Qwen3-ASR-1.7B, whisper-large-v3-turbo(와 구별되지 않는 Qwen3-ASR-0.6B), whisper-small, whisper-base 를 계층마다 시작 모델을 고르는 근거로 쓴다, 근거 4-2 의 같은 발화 64개 합산과 같은 발화 쌍 차이, 정확도 중간(Qwen3-ASR-1.7B 와 large-v3-turbo 의 차이는 표준오차의 약 2.7배라 기울지만 확정이 아니다)
  - 2-2절의 가설과 비교: "Qwen3-ASR-1.7B 가 가장 낮다" 는 맞는 쪽으로 기울고 확정은 아니다, "whisper-large-v3-turbo 가 그다음" 은 맞고 Qwen3-ASR-0.6B 와는 구별되지 않았다, "whisper-small 과 base 는 크게 높다" 는 맞다
- 영어 오류율(Q2): 판정, 잰 모델 모두 같은 영어 40개(Qwen3-ASR-0.6B 는 8개)에서 CER 이 6% 아래(whisper-base 가 5.30% 로 가장 높다)라 낭독 음성에서는 영어 때문에 뺄 후보가 없다, 근거 4-2, 정확도 중간(40개, 낭독 음성), whisper-large-v3 는 재지 못했다
- LoRA 역전파 한 번(Q3): 판정, 글 디코더만 붙이는 LoRA 는 Apple GPU 에서 세 모델 모두 돈다(whisper-small 과 whisper-large-v3-turbo 묶음 4, Qwen3-ASR-1.7B 묶음 1), 음성 인코더까지 붙이는 범위는 whisper-small 묶음 4 가 32분 넘게 끝나지 않았다(메모리 한도에 닿은 것으로 추측), 그래서 라벨 없는 FL 모의실험은 글 디코더 LoRA 로 시작한다(권고), 근거 4-2, 정확도 중간(손실이 숫자이고 기울기가 유한한 것과 시간과 메모리는 확인했고, 실제로 손실이 내려가는지는 확인하지 못했다)
### 5-9. 이 이슈의 결론
- 후보의 한국어 오류율 순서를 냈다(Qwen3-ASR-1.7B, large-v3-turbo 와 Qwen3-ASR-0.6B, small, base), 데스크톱 계층은 Qwen3-ASR-1.7B 를 첫 후보로 whisper-large-v3-turbo 를 대안으로, 폰 계층은 Qwen3-ASR-0.6B 와 whisper-small 을 후보로 두고, 시작 모델은 속도를 잰 뒤 메인이 정한다(프로젝트 결정 "시작 모델"), 글 디코더만 붙이는 LoRA 가 Apple GPU 에서 세 모델 모두 돈다, `issue_decision.md` D1 에서 D16, 근거 문서 `research/results/2026_11_19_start_model_cer.md`

## 6. 사용자 승인
- 해당 없음: 측정이고 새 사람 몫의 결정이 없다, 시작 모델을 고르는 일은 프로젝트 PD9 에서 한다

## 7. 할 일과 검증
(순서대로 한다, 시간 한도: 모델 하나의 측정이 25분을 넘을 것 같으면 표본을 줄이고 줄인 수를 적는다, 이 이슈 전체는 2시간 30분 안에 끝낸다)
(다른 worker 가 같은 노트북에서 Rust 를 빌드한다, 메모리가 16GB 라 모델은 한 번에 하나만 올린다)
- [x] 7-1. 환경: `research/` 에 uv 프로젝트(`pyproject.toml`, Python 3.12, torch, transformers, datasets, 오류율 계산 라이브러리, ruff, pytest)를 만든다, `research/Makefile` 에 `lint`(ruff), `test`(pytest), `cer`(측정) target 을 만들고 모든 target 이 `HF_HOME=/Users/min-jo/Documents/Developments/seamone/.tmp/hf` 를 쓰게 한다, 가상환경은 `research/.venv` / 검증: `make -C research lint`, `make -C research test` / 결과: `research/pyproject.toml:5-24`(Python 3.12, 의존성은 board 와 달리 datasets 를 뺐고 D1, `:31-38` 에서 qwen-asr 의 데모용 의존성을 뺐다 D2), `research/uv.lock`(패키지 version 은 이 파일이 원본), `research/Makefile:7-8`(HF_HOME 을 git 공통 폴더에서 찾은 메인 레포의 `.tmp/hf` 로 정한다 D7)와 target install, eval-ids, lint, test, prefetch, cer, score, compare, lora(`:28-69`), `research/.gitignore`, `research/README.md`, 가상환경 `research/.venv` 1.3GB, 검증: `make -C research lint` 통과(ruff 검사와 형식 검사), `make -C research test` 51개 통과(07:12, 검토 지적 반영 뒤 111개 통과 08:18, 두 번째 검토 반영 뒤 129개 통과 08:43, 라벨 없는 FL 모의실험의 시험 28개 포함)
- [x] 7-2. CER 계산: `research/seamone_research/cer.py`, 정규화는 유니코드 NFC, 소문자, 문장부호와 공백 제거, 숫자는 바꾸지 않는다(정답은 "오" 이고 모델은 "5" 로 쓴 경우 오류로 잡히는 한계를 결과 문서에 적는다), CER 은 세트 전체의 편집 거리 합을 정답 글자 수 합으로 나눈다 / 검증: 단위 시험(같은 글 0, 공백과 문장부호만 다른 글 0, 한 글자 바뀜, 빈 가설, 한글과 영어 섞임) / 결과: `research/seamone_research/cer.py:26` normalize(NFC, 소문자, 범주 P*, Z*, Cc, Cf 제거, 숫자와 S* 는 둔다 D5), `:65` count_errors(jiwer 로 대체, 삭제, 삽입 수), `:84` corpus_counts(세트 전체 합), `:91` ratio_estimate 와 `:104` corpus_standard_error(표준오차), 검증: `research/tests/test_cer.py`(같은 글 0, 공백과 문장부호만 다른 글 0, 한 글자 바뀜, 빈 가설, 한글과 영어 섞임, 숫자 안 바꿈, 세트 합산 등 18개 함수), 시험이 결함을 잡는지는 scratchpad 사본에서 일곱 가지 변이(정답 글자 수를 안 더함, 문장부호를 안 지움, 기호까지 지움, NFC 안 함, 30초 안 거름, 문장마다 하나로 안 줄임, 공통 앞부분을 가장 긴 쪽으로 잡음)로 확인했고 하나는 시험을 고쳐 잡게 했다
- [x] 7-3. 시험 세트: `research/seamone_research/data.py`, FLEURS `ko_kr` test 200개, `en_us` test 100개, Zeroth-Korean test 200개를 seed 고정으로 뽑아 16kHz mono 로 맞춘다, 뽑은 id 목록을 `research/data/eval_ids.json` 으로 커밋하고 음성은 `.tmp/hf` 에만 둔다, 세트 전체를 받지 않고 test split 만 받는다 / 검증: id 목록의 수와 총 음성 길이를 출력해 적는다 / 결과: `research/seamone_research/data.py:93` select_ids(D4), `:164` build_eval_ids, `:260` prepare_audio(test split 파일만 받아 뽑힌 녹음을 16kHz mono wav 로 `.tmp/hf/seamone_eval/` 에 두고 원본은 지운다), `:283` load_eval_set, `research/data/eval_ids.json`(540줄), 검증: `make -C research eval-ids` 출력이 fleurs_ko 200개 2569.0초(후보 270문장), fleurs_en 100개 993.7초(후보 350문장), zeroth_ko 200개 1934.7초(후보 457개), 음성 파일 500개 합계 169MB
- [x] 7-4. 추론: `research/seamone_research/transcribe.py`, Whisper 계열은 transformers, Qwen3-ASR 은 공식 패키지로 같은 interface(음성 목록을 받아 글 목록을 낸다) 뒤에 둔다, 언어는 세트의 언어로 지정하고 greedy 로 푼다, 가설 글은 `.tmp/research/hyp/<모델>/<세트>.jsonl` 에 둔다 / 검증: 모델마다 음성 5개로 먼저 돌려 글이 나오는지 본다 / 결과: `research/seamone_research/transcribe.py:24` Transcriber(음성 목록과 언어를 받아 글 목록을 낸다), `:30` WhisperTranscriber(transformers), `:60` QwenAsrTranscriber(qwen-asr, 언어 이름 Korean 과 English), `research/seamone_research/models.py:11` 모델마다 device 와 dtype 과 묶음 크기, `research/seamone_research/measure.py:73` measure_set(가설 글을 eval_ids 순서로 jsonl 에 쌓고 이어 쓴다 D6)와 `:32` LazyTranscriber(처음 쓸 때만 모델을 불러온다, 끝나면 `memory.py:11` release_memory 로 메모리를 돌려준다), 검증: whisper-base 는 `N=5` 로, Qwen3-ASR-0.6B 는 3개로 먼저 돌려 글이 나오는 것을 봤고, 나머지 모델은 첫 묶음의 가설 글을 정답과 눈으로 대조했다(글이 정상이고 환각이 없었다)
- [ ] 7-5. 측정: 작은 모델부터(base, small, turbo, Qwen 0.6B, Qwen 1.7B, large-v3) 돌려 세트마다 CER 과 걸린 시간을 적는다, 한 모델이 끝날 때마다 `df -h` 의 여유를 적고, #4 에 쓰지 않는 가중치(base, large-v3, 그리고 Qwen 두 크기 가운데 오류율이 높은 쪽)는 그 자리에서 `.tmp/hf` 에서 지운다, 남기는 것은 whisper-small, large-v3-turbo, Qwen 가운데 가장 좋은 것 / 검증: `make -C research cer` 가 표본 5개짜리 빠른 모드로 끝까지 돈다 / 진행(07:10): whisper-base 와 whisper-small 은 세 세트 전부, whisper-large-v3-turbo 는 56/40/16개, Qwen3-ASR-1.7B 는 56/40/8개, Qwen3-ASR-0.6B 는 19/8/8개(한국어 FLEURS, 영어 FLEURS, Zeroth 순서)를 쟀다, 세트마다 CER 과 걸린 시간은 결과 문서 3절, 지운 가중치는 whisper-base(측정이 끝난 뒤)와 Qwen3-ASR-0.6B(06:03)와 whisper-large-v3(06:03), 남긴 가중치는 whisper-small, whisper-large-v3-turbo, Qwen3-ASR-1.7B(`.tmp/hf` 7.0GB), 남은 것: whisper-large-v3 를 재지 못했다(D12, 디스크와 메모리), 빠른 모드 검증은 whisper-base 로 04:36 에 `N=5` 가 끝까지 돌았고(묶음 크기와 불러오기 방식을 바꾸기 전 코드), 지금 코드로는 06:35 부터 07:05 까지 돈 `make -C research cer` 실행들이 모델 불러오기, 가설 글 쌓기, 채점까지 끝까지 돌았다(로그 `.tmp/research/cer-*.log`, 이 갈음은 D16)
- [ ] 7-6. 학습 가능성: 남긴 모델마다 LoRA 를 붙여 음성 4개 한 묶음으로 순전파와 역전파 한 번을 돌리고 성공 여부, 걸린 시간, 메모리 최고치를 적는다(Apple GPU 와 CPU 둘 다) / 검증: 손실 값이 숫자로 나온다 / 진행(07:10): 글 디코더만 붙이는 범위는 Apple GPU 에서 whisper-small 묶음 4, whisper-large-v3-turbo 묶음 4, Qwen3-ASR-1.7B 묶음 1 이 성공했고 Qwen3-ASR-1.7B 묶음 4 는 GPU 메모리 한도를 넘어 실패했다, 범위 all 은 whisper-small 이 묶음 1 에서 성공했고 묶음 4 는 32분 넘게 끝나지 않아 06:20 에 멈췄다, CPU 는 whisper-small 만 쟀다(범위 decoder 와 all), 남은 것은 whisper-large-v3-turbo 와 Qwen3-ASR-1.7B 의 범위 all 과 두 모델의 CPU 시험(부하 때문에 07:00 에 뺐다), 결과 문서 4절
- [x] 7-7. 결과 문서 `research/results/2026_11_19_start_model_cer.md` 를 쓰고 이 board 4절과 5절을 채운다, 표를 쓰지 않고 목록으로 쓴다, 추천에는 데스크톱 계층과 폰 계층 후보를 나눠 적되 속도는 #3 에서 잰다고 밝힌다 / 결과: `research/results/2026_11_19_start_model_cer.md`(조건 2절, 결과 3절, LoRA 4절, 한계 5절, 추천 6절, 다시 돌리는 법 7절)를 썼고 이 board 4절과 5절을 07:10 기준으로 채웠다, 추천에 데스크톱 계층(Qwen3-ASR-1.7B 첫 후보, whisper-large-v3-turbo 대안)과 폰 계층(Qwen3-ASR-0.6B, whisper-small)을 나눠 적고 속도는 받아쓰기 엔진 이슈가 잰다고 밝혔다
- [x] 7-8. 8절 수정 요약을 쓰고 보고를 쓴다, 보고에 지금 `.tmp/hf` 와 `research/.venv` 의 크기를 적는다 / 결과: 8절을 썼고 보고는 `issue_log.md` 와 `.tmp/report-1.md` 에 썼다, 크기는 `.tmp/hf` 7.0GB 와 `research/.venv` 1.3GB(07:22)
- [x] 7-9. [중간, 결함] 준비한 파일보다 넓은 공유 cache 를 통째로 지운다 / 무엇이: 옛 `data.py` 의 `_drop_hub_cache` 가 `HF_HOME/hub/datasets--<저장소>` 를 통째로 `rmtree` 해 같은 저장소의 다른 파일(학습 조각 등)을 지웠다 / 결과: `research/seamone_research/data.py:149-166` `raw_cache_dir`, `remove_inside`(지울 폴더가 전용 폴더 안인지, 루트 자신이나 바깥이나 바깥으로 이어진 link 가 아닌지 검사한다), `drop_raw_cache` 로 바꿨고 `:169` `_download` 가 세트마다 전용 cache(`.tmp/hf/eval-raw/<세트>`)로 받는다, 옛 함수는 지웠다, `research/seamone_research/fl_data.py:61` 의 FL 기기 데이터도 전용 폴더(`.tmp/hf/fl-raw`)로 받고 그 폴더만 지운다 / 검증: `research/tests/test_data_cache.py`(가짜 `hub/datasets--kresnik--zeroth_korean/` 파일이 준비 뒤에도 남고 전용 폴더는 지워진다, 내려받기가 `cache_dir` 로 전용 폴더를 쓴다, `remove_inside` 가 루트와 바깥과 바깥으로 이어진 link 를 거부한다)
- [x] 7-10. [중간, 결함] 중단으로 마지막 줄이 잘린 가설 파일에서 측정을 재개하지 못한다 / 결과: `research/seamone_research/hypotheses.py:17` `_parse`, `:40` `read_hypotheses`(끝 줄이 깨졌으면 그 줄을 빼고 읽는다, 중간 줄이 깨졌으면 `<파일>:<줄 번호> 줄이 깨졌다` 오류로 멈춘다), `:45` `repair_hypotheses`(이어 쓰기 전에 깨진 끝 줄을 버리고 끝 개행을 맞춰 임시 파일로 쓴 뒤 이름을 바꿔 마지막 완성 줄까지 자른다), `research/seamone_research/measure.py` 의 `measure_set` 이 이어 쓰기 전에 `repair_hypotheses` 를 부른다 / 검증: `research/tests/test_hypotheses.py` 일곱 개(정상, 끝 줄 잘림 뒤 이어 쓰기, 다중 바이트 문자 안에서 잘림, 중간 줄 깨짐의 줄 번호, 빈 파일과 없는 파일, 끝 개행 없는 완성 줄, 빈 줄)
- [x] 7-11. [중간, 결함] LoRA 성공 판정이 기울기 없는 가중치와 비유한 손실을 통과시킨다 / 결과: `research/seamone_research/lora_probe.py:73-80` `ProbeJudgement` 와 `judge_probe`(모든 학습 가능한 parameter 에 기울기가 있고 모두 유한하며 모든 걸음의 손실이 유한할 때만 성공), `run_probe`(`:163`)가 학습 가능한 parameter 마다의 기울기(없으면 None)와 걸음마다의 손실을 넘기고 결과 파일에 `params_without_grad` 와 `losses_finite` 를 적는다 / 검증: `research/tests/test_lora_probe.py` 에 다섯 개(모두 정상, 기울기 None 하나, NaN 손실과 무한 손실, 무한 기울기, 기울기나 손실이 비었을 때), 이미 쓴 결과 문서의 LoRA 절은 새 기준으로 Apple GPU 시험 넷(범위 decoder 의 whisper-small 묶음 4, whisper-large-v3-turbo 묶음 4, Qwen3-ASR-1.7B 묶음 1, 범위 all 의 whisper-small 묶음 1)을 08:14 에서 08:20 에 다시 돌려 확인했다, 넷 모두 성공이고 기울기 없는 가중치 0개, 손실 모두 유한이라 문서의 결론은 바뀌지 않았고 결과 문서 4절에 새 기준과 다시 돌린 결과를 적었다, whisper-small CPU 시험 둘은 다시 돌리지 않아 새 기준으로 확인하지 못했다
- [x] 7-12. [중간, 결함] 모델 판을 고정하지 않아 재실행 결과가 섞인다 / 결과: `research/seamone_research/models.py:13` `ModelSpec` 에 `revision`(결과 문서 2-1절의 커밋)을 한 번만 두고 `:79` `resolve_model_path` 가 그 커밋으로 `snapshot_download` 한 폴더 경로를 돌려준다, Whisper 와 Qwen3-ASR 의 로더(`research/seamone_research/transcribe.py:30,60,85`), `prefetch.py`, `lora_probe.py`, `fl_train.py` 가 모두 이 경로로 불러온다(`qwen_asr` 의 `from_pretrained` 가 revision 을 받는지 확인하지 않고 폴더 경로로 불러오는 쪽으로 정했다), `research/seamone_research/measure.py:77` `measurement_config` 와 `:91` `ensure_same_config` 가 가설 글 옆 `<세트>.config.json` 에 측정 설정(모델 repo 와 revision, 데이터 revision, 정규화 판 `cer.py:26`, device, dtype)을 쓰고 이어 쓸 때 대조해 다르면, 또는 가설이 있는데 설정이 없으면 멈춘다 / 검증: `research/tests/test_models_revision.py`(커밋 40자, `snapshot_download` 에 revision 이 간다, 가짜 로더로 Whisper 와 Qwen 로더가 해석된 폴더에서 불러온다), `research/tests/test_measure_config.py`(설정이 다르면 재개를 거부한다, 설정 없이 가설만 있으면 거부한다), `ls` 확인: 남아 있는 가중치의 snapshot 폴더 이름이 결과 문서 2-1절의 커밋과 같다(whisper-small 973afd24965f72e36ca33b3055d56a652f456b4d, whisper-large-v3-turbo 41f01f3fe87f28c78e2fbf8b568835947dd65ed9, Qwen3-ASR-1.7B 7278e1e70fe206f11671096ffdd38061171dd6e5), 지운 가중치 셋(whisper-base, Qwen3-ASR-0.6B, whisper-large-v3)은 확인하지 못했다, 이미 쌓은 가설 글 15개 파일 옆에는 문서의 커밋과 device mps, dtype float16 으로 설정 파일을 한 번 써 넣었다(`.tmp/research/adopt_configs.py`, 버려도 되는 도구)
- [x] 7-13. [중간, 결함] 음성 cache 재사용이 데이터 판과 표본을 검사하지 않는다 / 결과: `research/seamone_research/data.py:292` `manifest_for`(데이터 repo 와 revision, id 수, id 목록의 SHA-256), `:310` `cache_is_valid`(manifest 대조, 정답 파일이 해석되는지, 필요한 id 의 정답과 wav 가 모두 있는지), `:325` `prepare_audio` 가 같은 판의 완성본이 있을 때만 재사용하고 아니면 manifest 를 지운 뒤 다시 만들고 manifest 를 마지막에 쓴다, wav 와 정답 파일과 manifest 는 `.part` 임시 이름으로 쓴 뒤 이름을 바꾼다(`:251` `_save_wav`, `:261` `_write_json_atomically`), 이미 준비된 세 음성 폴더에는 파일이 완전한지 확인한 뒤 manifest 를 한 번 써 넣었다(`.tmp/research/adopt_manifest.py`, 버려도 되는 도구) / 검증: `research/tests/test_data_cache.py`(다른 데이터 revision 과 다른 id 목록과 잘린 정답 파일과 빠진 wav 와 정답과 manifest 없음은 거부하고 정상은 다시 받지 않는다, `_save_wav` 가 완성된 파일만 남긴다)
- [x] 7-14. [낮음, 방향] 결과 문서의 "같은 말을 되풀이하는 환각은 없었다" 가 길이 검사만으로 단정한다 / 결과: `research/results/2026_11_19_start_model_cer.md` 3-4절을 "정답 길이의 두 배를 넘는 가설은 다섯 모델 모두 0개였다, 정규화한 글자 수만 견준 검사라 짧은 구절을 되풀이한 가설은 이 검사로 잡히지 않는다(반복이 없었다는 증거는 아니다)" 로 한정했고 `research/seamone_research/score.py:31` `runaway_count` 의 docstring 에서 "되풀이하는 환각" 을 뺐다 / 검증: `rg -n '되풀이' research/` 로 남은 단정이 없음을 확인했다(결과 문서의 한정 문장 한 곳만 걸린다)
- [x] 7-15. [중간, 결함] 같은 세트를 둘 이상의 실행이 동시에 준비하면 서로의 원본을 지우거나 파일이 섞인다 / 결과: `research/seamone_research/data.py:167` `set_lock`(세트별 잠금 파일 `.tmp/hf/eval-raw/<세트>.lock` 에 `fcntl.flock`), `:353` `prepare_audio` 가 잠금 안에서 `:364` `_build_audio`(받기, wav 와 정답 쓰기, manifest 쓰기, 원본 지우기)를 하고 잠금을 쥔 뒤 `cache_is_valid` 를 다시 본다(기다리는 동안 다른 실행이 끝냈으면 만들지 않는다), `:308` `wav_sizes_of` 와 `:314` `manifest_for` 가 wav 파일 이름별 크기를 manifest 에 담고 `:333` `cache_is_valid` 가 실제 크기와 대조한다, 이행: 이미 준비된 세 음성 폴더의 manifest 를 크기가 담긴 꼴로 다시 썼다(`.tmp/research/adopt_manifest.py`, 파일이 모두 있는지 확인한 뒤) / 검증: `research/tests/test_data_cache.py` 에 여섯 개 더(wav 크기가 다르면 거부, 크기 없는 manifest 거부, 스레드 둘로 같은 세트의 두 번째 준비가 기다린다, 다른 세트는 기다리지 않는다, `prepare_audio` 가 만드는 동안 잠금을 쥔다, 기다린 준비는 다시 만들지 않는다), 일부러 어긴 구현 넷(scratchpad 사본)을 모두 시험이 잡았다: 잠금 없이 만들기, wav 크기 대조 빼기, 잠금 뒤 재확인 빼기, 잠금이 잠그지 않기
- [x] 7-16. [중간, 결함] 측정 설정 파일을 직접 써서 쓰기가 중단되면 가설이 없어도 재시작이 막힌다 / 결과: `research/seamone_research/measure.py:103` `_read_config` 와 `:118` `ensure_same_config` 가 설정을 `_write_json_atomically`(`.part` 로 쓴 뒤 이름을 바꾼다)로 쓰고, 설정이 해석되지 않으면 가설 행이 없을 때 새로 쓰며 가설 행이 있으면 파일 경로와 JSON 오류 위치를 적은 오류로 멈춘다 / 검증: `research/tests/test_measure_config.py` 에 일곱 개 더(잘린 설정과 가설 없음은 새로 만든다, 잘린 설정과 가설 있음은 파일 이름을 적은 오류, 객체가 아닌 설정, 임시 이름으로 쓴다, 표본 목록 해시, 순서가 바뀐 표본 거부, 앞부분 검사)와 `research/tests/test_measure_resume.py` 다섯 개(`measure_set` 전체 경로: 잘린 설정과 가설 없음의 재시작 포함), 일부러 어긴 구현 둘을 시험이 잡았다: 설정을 직접 `write_text` 로 쓰면 임시 이름 시험이 실패했고, 가설이 없어도 깨진 설정에서 멈추게 하면 재시작 시험이 실패했다
- [x] 7-17. [중간, 결함] 표본 목록이나 순서가 바뀌어도 측정 설정 대조를 통과한다 / 결과: `research/seamone_research/measure.py:80,85` `ids_fingerprint` 와 `measurement_config` 가 설정에 `ids_count` 와 세트 전체 id 목록의 해시 `ids_sha256` 를 넣고(`limit` 와 무관), `:145` `ensure_prefix_of_current_order` 와 `measure_set`(`:152`)이 이어 쓰기 전에 쌓인 가설의 id 가 지금 표본 순서의 앞부분인지 검사한다(`N` 을 늘리는 재개는 통과한다), 이행: 이미 쌓은 가설 글 15개 파일 옆 설정 파일에 같은 값을 한 번 써 넣었고(`.tmp/research/adopt_ids_hash.py`, 쓰기 전에 가설 id 가 지금 표본 순서의 앞부분인지와 기존 설정의 다른 값이 그대로인지 확인했다) `make -C research score` 가 그대로 돈다 / 검증: `research/tests/test_measure_resume.py`(N 을 늘린 재개 허용, 같은 표본은 더 할 일 없이 통과, 순서가 바뀐 표본은 거부하고 아무것도 쓰지 않는다, 설정이 맞아도 현재 순서의 앞부분이 아닌 가설이 있으면 거부), 일부러 어긴 구현 셋을 시험이 잡았다: 설정에서 해시를 빼기, 앞부분 검사를 부르지 않기, 앞부분 검사가 항상 통과하기

## 8. 수정 요약
- `research/pyproject.toml`: 새 파일, uv 프로젝트(Python 3.12 만, torch, transformers 4.57.6, qwen-asr 0.0.6, peft, jiwer 등 의존성과 ruff, pytest 설정), qwen-asr 의 데모용 의존성을 설치하지 않게 뺌
- `research/uv.lock`: 새 파일, 패키지 version 고정
- `research/Makefile`: 새 파일, target install, eval-ids, lint, test, prefetch, cer, score, compare, lora 와 `HF_HOME` 을 메인 레포의 `.tmp/hf` 로 정함, 측정 명령에 `nice -n 10`
- `research/.gitignore`: 새 파일, `.venv` 와 캐시를 git 에서 뺌
- `research/README.md`: 새 파일, make target 과 폴더 설명
- `research/data/eval_ids.json`: 새 파일, 세 세트의 표본 id 500개(seed 20261119)와 후보 수와 음성 합계
- `research/seamone_research/__init__.py`, `research/tests/__init__.py`: 새 빈 파일
- `research/seamone_research/paths.py`: 새 파일, `HF_HOME` 과 가설 글 폴더 자리
- `research/seamone_research/cer.py`: 새 파일, 글 정규화, CER, 표준오차(비율 추정)
- `research/seamone_research/data.py`: 새 파일, 표본 뽑기, 음성 준비, 세트별 N 해석
- `research/seamone_research/models.py`: 새 파일, 후보 모델 여섯의 repo, 묶음 크기, device, dtype
- `research/seamone_research/transcribe.py`: 새 파일, Whisper 와 Qwen3-ASR 의 같은 interface 받아쓰기
- `research/seamone_research/hypotheses.py`: 새 파일, 가설 글 파일 읽기와 쓰기
- `research/seamone_research/measure.py`: 새 파일, 모델 하나의 받아쓰기 실행과 시간 기록
- `research/seamone_research/score.py`: 새 파일, 저장된 가설 글의 CER 과 공통 앞부분 CER
- `research/seamone_research/compare.py`: 새 파일, 같은 발화에서 세트 합산 CER 과 모델 둘의 CER 차이
- `research/seamone_research/memory.py`: 새 파일, 메모리 재기와 모델을 내린 뒤 메모리 돌려주기
- `research/seamone_research/prefetch.py`: 새 파일, 가중치 미리 받기(safetensors 와 설정 파일만)
- `research/seamone_research/lora_probe.py`: 새 파일, LoRA 순전파와 역전파 시험(범위 all 과 decoder)
- `research/tests/test_cer.py`, `test_data.py`, `test_score.py`, `test_compare.py`, `test_lora_probe.py`: 새 파일, 단위 시험
- `research/results/2026_11_19_start_model_cer.md`: 새 파일, 결과 문서
- 이슈 기록: 이 board 의 4, 5, 7, 8절, `issue_decision.md` D1 에서 D16, `issue_log.md` 의 보고, `.tmp/report-1.md`
- 검토 지적 반영(7-9 에서 7-14): `research/seamone_research/data.py`(전용 cache 와 안전한 삭제, manifest 와 임시 이름 쓰기), `hypotheses.py`(끝 줄 복구), `lora_probe.py`(성공 판정 함수), `models.py`(커밋 고정과 snapshot 경로), `transcribe.py`와 `prefetch.py`와 `fl_train.py`(그 경로로 불러오기), `measure.py`(측정 설정 대조), `cer.py`(정규화 판 이름), `score.py`(docstring), 결과 문서 3-4절과 4절, 새 시험 `research/tests/test_hypotheses.py`, `test_data_cache.py`, `test_models_revision.py`, `test_measure_config.py`, `test_lora_probe.py` 에 다섯 개 더
- 두 번째 검토 반영(7-15 에서 7-17): `research/seamone_research/data.py`(세트별 잠금, wav 크기를 담은 manifest), `measure.py`(설정을 임시 이름으로 쓰기, 깨진 설정 처리, 표본 목록 해시와 앞부분 검사), 시험 `research/tests/test_measure_resume.py` 새 파일, `test_measure_config.py` 에 일곱 개와 `test_data_cache.py` 에 여섯 개 더
- 지운 것: `.tmp/hf` 의 whisper-base, Qwen3-ASR-0.6B, whisper-large-v3 가중치(재측정하려면 다시 받는다)

## 9. 지금 상태
- 메인이 직접 확인한 것: 브랜치가 main 에 merge 됐다(merge 커밋 7d069d4, 2026-11-19 08:52), `make -C research lint`, `make -C research test` EXIT=0 을 세 번(51개, 111개, 129개 통과), 저장된 가설 글에서 내 독립 코드로 센 오류율이 보고와 같다(Qwen3-ASR-1.7B 한국어 56개 4.53%, whisper-large-v3-turbo 5.83%, whisper-small 앞 56개 7.65%), 삭제 범위 검사, 잘린 가설 파일 복구, 세트별 잠금 코드를 읽었다
- worker 주장(메인이 다시 하지 않은 것): Apple GPU 에서 LoRA 역전파 한 번의 시간과 GPU 메모리, 모델별 걸린 시간, 일부러 어긋난 구현에서 시험이 실패하는 확인(변이 일곱 가지와 아홉 가지)
- 남은 것: whisper-large-v3 측정, 표본 늘리기(Zeroth-Korean 60개), 속도 측정(받아쓰기 엔진 이슈)
- 판단거리: 시작 모델 결정은 속도를 잰 뒤(프로젝트 PD9)

## 10. Closing
### 10-1. 검토
- Codex Sol(`gpt-6.1-sol`) high 읽기 전용 두 번: 첫 검토(07:55 시작 08:03 끝)는 지적 여섯 개(낮음 하나, 중간 다섯)를 모두 원문으로 확인해 받아 고쳤다(커밋 6d642da), 두 번째 검토(08:31 시작 08:36 끝)는 이전 지적 여섯 개를 고쳐짐 넷과 일부 둘로 판정하고 새 지적 셋(중간)을 냈다, 모두 원문으로 확인해 받아 고쳤다(커밋 398e2fb), 검토 반복은 두 번에서 멈추는 규칙(review skill 4절)이라 세 번째 검토 대신 메인이 시험과 diff 로 확인했다, 해당 없음으로 둔 지적은 없다, 검토자는 `make -C research test` 를 sandbox 의 uv 잠금 파일 쓰기 거부로 돌리지 못했다, 원문은 `.tmp/review/codex-2.md`, `codex-4.md`
### 10-2. 커밋 직전 증거 대조
- 검증한 판본: 이슈 브랜치 398e2fb, 그때의 기준 브랜치 87db078
- 마지막 수정 뒤 시험: 08:51 메인이 `make -C research lint` EXIT=0, `make -C research test` 129개 통과
- 요구마다(3-1): R1 일부(여섯 후보 가운데 whisper-large-v3 는 재지 못했고 까닭이 결과 문서와 보고에 있다, 나머지 다섯은 같은 표본과 같은 정규화로 쟀다, 표본 수는 지시한 200 보다 적다: 한국어 FLEURS 19에서 56개, 영어 40개, Zeroth 8에서 16개, 시간과 메모리 때문이라고 보고에 있고 결과 문서가 표준오차를 함께 적었다), R2 변경에 있음(`make -C research cer`, 빠른 모드 N=5 는 이번 코드로 새로 돌리지 않고 이미 돈 실행으로 갈음했다는 worker 의 결정 D16), R3 변경에 있음(오류율 계산 시험), R4 변경에 있음(LoRA 역전파 시험), R5 변경에 있음(가중치와 데이터를 `.tmp/hf` 한 곳에 두고 쓰지 않는 것을 지움), 요구하지 않은 동작은 `score`, `compare` 와 일부러 어긴 구현 확인, 이행 스크립트(`.tmp/research/adopt_*.py`)
- 최종 방안대로 했는지: 같음(PD11 실험은 PyTorch, PD12 디스크)
- 수정 요약(8절)이 실제 diff 의 파일과 같은지: 같음, 시험 파일 일곱 개는 8절이 묶어서 적은 차이다
- 완료 기준마다(3-4): 통과(재지 못한 후보 하나는 까닭이 적혀 있어 기준을 만족한다), 산출물 `research/results/2026_11_19_start_model_cer.md` 와 `make -C research cer` 가 있다
- 결과: 통과
### 10-3. merge 직전 재확인
- 기준 브랜치는 10-2 의 판본 뒤로 앞 이슈의 merge(Rust 코드)와 기록 커밋이 늘었고 이 브랜치와 겹치는 파일이 없어 증거를 깨지 않는다, merge 충돌은 없었다
### 10-4. 사람 시험
- 해당 없음: 측정 결과는 명령으로 확인한다
### 10-5. 자원
- 받은 가중치: `.tmp/hf` 의 whisper-small, whisper-large-v3-turbo, Qwen3-ASR-1.7B(6.8GB)는 마무리 점검에서 지운다(다시 받을 수 있다, `make -C research prefetch`), 이미 지운 것은 whisper-base, Qwen3-ASR-0.6B, whisper-large-v3, worktree `/Users/min-jo/orca/workspaces/seamone/2-start-model-cer` 와 `research/.venv`(1.3GB)는 라벨 없는 FL 모의실험이 같은 worktree 를 써서 남겼다(지울 조건: 그 일을 커밋하고 worktree 를 정리할 때), 가설 글(`.tmp/research/hyp/`, 496KB)은 다시 만들 수 없는 측정 증거라 남긴다
### 10-6. 이슈 마무리 코멘트
- 상태: 게시함 2026-11-19 https://github.com/guny524/seamone/issues/2#issuecomment-6995145468
- 관련 MR: https://github.com/guny524/seamone/pull/7 (merge 됨)
### 10-7. 끝내기 승인
- 2026-11-19 메인 판단(사람 시험이 없고 명령으로 검증이 끝나는 이슈), 근거는 10-2, 대상 merge 커밋 7d069d4
### 10-8. 커밋과 merge
- 65bb8c4 feat: 시작 모델 후보의 오류율 측정 코드와 결과 문서를 추가, 6d642da fix: 측정 코드의 cache 삭제 범위, 판 고정, 재개와 LoRA 판정을 검토대로 고침, 398e2fb fix: 측정 코드의 준비 잠금, 설정 원자적 쓰기, 표본 목록 대조를 검토대로 고침, merge 7d069d4(2026-11-19)

## 11. 참고
- Qwen3-ASR 모델 카드 https://huggingface.co/Qwen/Qwen3-ASR-1.7B, 쓰는 법과 지원 언어
- FLEURS https://huggingface.co/datasets/google/fleurs, 여러 언어 낭독 음성(CC BY 4.0)
- Zeroth-Korean https://huggingface.co/datasets/kresnik/zeroth_korean, 한국어 낭독 음성(CC BY 4.0), 화자 id 가 있어 #4 에서 기기로 나눈다
