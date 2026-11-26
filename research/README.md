# research
- seamone 모델 후보의 오류율과 로컬 학습 계산을 확인하는 PyTorch 연구 코드다, 앱에는 들어가지 않는다
- Python 3.12 와 uv, 가상환경은 `research/.venv`

## 1. make target
- `make -C research install`: 의존성을 설치한다
- `make -C research lint`: 삭제 경계 AST 검사와 행동 시험, ruff 검사와 형식 검사
- `make -C research test`: lint 를 돌린 뒤 pytest
- `make -C research test-offline`: CI의 Linux runner에서 외부 네트워크를 분리한 뒤 같은 lint와 pytest를 실행한다. 먼저 `uv sync --locked --project research`로 가상환경을 준비한다. `sudo` 권한으로 네트워크 namespace를 만들고 원래 사용자로 돌아와 loopback 인터페이스만 있는지 확인한다
- `make -C research eval-ids`: 시험 세트의 표본 id 목록(`data/eval_ids.json`)을 만들고 음성을 `.tmp/hf` 에 둔다
- `make -C research prefetch MODELS="whisper-small ..."`: 모델 가중치를 미리 받는다
- `make -C research cer`: 후보 모델의 글자 오류율(CER)을 잰다
  - `MODELS` 로 모델, `SETS` 로 세트를 고르고, `N` 은 세트마다 앞에서부터 잴 발화 수다(`N=5` 면 빠른 확인)
  - 모델마다 process 하나로 돌고 가설 글을 `.tmp/research/hyp/<모델>/<세트>.jsonl` 에 쌓는다, 멈췄다 다시 돌려도 이어 쓴다
  - 끝에 `score` 가 저장된 가설 글로 CER 을 다시 계산해 낸다
  - 가설 글 옆의 `<세트>.config.json` 이 측정 설정(모델과 데이터의 revision, 정규화 판, 표본 목록 해시)을 기록하고, 이어 쓸 때 대조해 다르면 멈춘다
- `make -C research score MODELS="..."`: 저장된 가설 글로 CER 을 다시 계산한다(모델을 돌리지 않는다)
- `make -C research compare MODELS="a b" SETS="fleurs_ko zeroth_ko" PAIR="a b"`: 같은 발화에서 세트들을 합친 CER 과 두 모델의 CER 차이를 낸다
- `make -C research lora LORA_MODELS=... LORA_DEVICES=mps`: LoRA 를 붙여 순전파와 역전파를 돌려 성공 여부와 시간과 메모리를 잰다
  - `LORA_BATCH`(기본 4)는 음성 묶음 크기, `LORA_SCOPE` 는 LoRA 를 붙일 범위(`all` 은 음성 인코더와 글 디코더 모두, `decoder` 는 글 디코더만)

## 2. 자리
- `seamone_research/cer.py`: 글 정규화와 CER 계산
- `seamone_research/data.py`: 시험 세트(FLEURS 한국어와 영어, Zeroth-Korean)의 표본 뽑기와 음성 준비
- `seamone_research/cache.py`: 전용 cache의 삭제 입구 `remove_inside`, HF_HOME 아래 eval-raw/seamone_eval 안에서만 root의 자식을 지우며 공유 hub는 거절한다, 소스와 시험의 직접 삭제 API는 AST 검사로 막고 경로와 symlink는 행동 시험으로 확인한다
- `seamone_research/models.py`, `transcribe.py`: 후보 모델과 같은 interface 의 받아쓰기
- `seamone_research/measure.py`, `hypotheses.py`: 받아쓰기 실행과 가설 글 파일 읽기와 쓰기
- `seamone_research/score.py`, `compare.py`: 채점, 같은 발화에서 모델끼리 견주기
- `seamone_research/lora_probe.py`: 학습 가능성 시험
- `seamone_research/paths.py`, `memory.py`, `prefetch.py`: 폴더 자리, 메모리 재기와 돌려주기, 가중치 미리 받기
- `data/eval_ids.json`: 표본 id 목록, git 에 둔다
- `results/`: 결과 문서
- 가중치와 시험 음성은 메인 레포의 `.tmp/hf` 한 곳에만 둔다(Makefile 이 `HF_HOME` 으로 정한다, worktree 마다 받지 않는다)

## 3. 사람 정답 fixture로 실제 optimizer 한 번 돌리기
- `make -C research prefetch MODELS=whisper-small`로 고정한 PyTorch 모델을 먼저 준비한다, `verified-step`은 로컬 cache의 revision만 읽는다
- `make -C research training-fixture`는 고정한 `google/fleurs` revision의 한국어 train split에서 짧은 발화 하나와 사람 정답을 받아 `.tmp/research/training-fixture/`에 둔다, archive를 끝까지 내려받지 않는다
- `make -C research verified-step PROOF_DEVICE=cpu`는 Whisper small decoder의 q/v projection에 LoRA를 붙여 AdamW optimizer step 한 번을 실행한다
- 기반 모델의 모든 고정 parameter를 전후 SHA-256으로 대조하고 adapter 변화량이 0보다 큰지 확인한다, `proof.json`과 adapter는 `.tmp/research/verified-step/`에 둔다
- `FIXTURE`와 `PROOF_DIR`로 입력 manifest와 출력 폴더를 지정한다, manifest는 공개 사람 정답의 train split만 받고 평가 ID와 충돌하거나 음성 SHA-256이 다르면 거절한다
- fixture와 정답, 모델 가중치, adapter는 git에 넣지 않는다, 이 입력은 앱의 검증이나 학습 대기열로 들어가지 않는다
- 이 시험은 학습 계산이 실행됐는지 확인한다, CER 개선이나 앱의 native 학습, GGML adapter 적용을 증명하지 않는다
