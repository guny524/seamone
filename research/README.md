# research
- seamone 의 모델 고르기와 라벨 없는 학습 실험 코드다, PyTorch 로 쓰고 앱에는 들어가지 않는다(프로젝트 결정 PD11)
- Python 3.12 와 uv, 가상환경은 `research/.venv`

## 1. make target
- `make -C research install`: 의존성을 설치한다
- `make -C research lint`: ruff 검사와 형식 검사
- `make -C research test`: lint 를 돌린 뒤 pytest
- `make -C research eval-ids`: 시험 세트의 표본 id 목록(`data/eval_ids.json`)을 만들고 음성을 `.tmp/hf` 에 둔다
- `make -C research prefetch MODELS="whisper-small ..."`: 모델 가중치를 미리 받는다
- `make -C research cer`: 후보 모델의 글자 오류율(CER)을 잰다
  - `MODELS` 로 모델, `SETS` 로 세트를 고르고, `N` 은 세트마다 앞에서부터 잴 발화 수다(`N=5` 면 빠른 확인)
  - 모델마다 process 하나로 돌고 가설 글을 `.tmp/research/hyp/<모델>/<세트>.jsonl` 에 쌓는다, 멈췄다 다시 돌려도 이어 쓴다
  - 끝에 `score` 가 저장된 가설 글로 CER 을 다시 계산해 낸다
- `make -C research score MODELS="..."`: 저장된 가설 글로 CER 을 다시 계산한다(모델을 돌리지 않는다)
- `make -C research compare MODELS="a b" SETS="fleurs_ko zeroth_ko" PAIR="a b"`: 같은 발화에서 세트들을 합친 CER 과 두 모델의 CER 차이를 낸다
- `make -C research lora LORA_MODELS=... LORA_DEVICES=mps`: LoRA 를 붙여 순전파와 역전파를 돌려 성공 여부와 시간과 메모리를 잰다
  - `LORA_BATCH`(기본 4)는 음성 묶음 크기, `LORA_SCOPE` 는 LoRA 를 붙일 범위(`all` 은 음성 인코더와 글 디코더 모두, `decoder` 는 글 디코더만)

## 2. 자리
- `seamone_research/cer.py`: 글 정규화와 CER 계산
- `seamone_research/data.py`: 시험 세트(FLEURS 한국어와 영어, Zeroth-Korean)의 표본 뽑기와 음성 준비
- `seamone_research/models.py`, `transcribe.py`: 후보 모델과 같은 interface 의 받아쓰기
- `seamone_research/measure.py`, `hypotheses.py`: 받아쓰기 실행과 가설 글 파일 읽기와 쓰기
- `seamone_research/score.py`, `compare.py`: 채점, 같은 발화에서 모델끼리 견주기
- `seamone_research/lora_probe.py`: 학습 가능성 시험
- `seamone_research/paths.py`, `memory.py`, `prefetch.py`: 폴더 자리, 메모리 재기와 돌려주기, 가중치 미리 받기
- `data/eval_ids.json`: 표본 id 목록, git 에 둔다
- `results/`: 결과 문서
- 가중치와 시험 음성은 메인 레포의 `.tmp/hf` 한 곳에만 둔다(Makefile 이 `HF_HOME` 으로 정한다, worktree 마다 받지 않는다)
