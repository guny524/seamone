# research/ 의 make target
# - lint: ruff 검사, test: lint 뒤 pytest, cer: 후보 모델의 오류율 측정, lora: LoRA 역전파 한 번의 시험
# - 모델 가중치와 시험 음성은 메인 레포의 .tmp/hf 한 곳에 둔다(worktree 마다 받지 않는다)

# 이 폴더가 worktree 여도 git 의 공통 폴더(<메인 레포>/.git)에서 메인 레포를 찾는다
GIT_COMMON_DIR := $(shell git rev-parse --path-format=absolute --git-common-dir)
HF_HOME := $(abspath $(dir $(GIT_COMMON_DIR)))/.tmp/hf
export HF_HOME
# MPS 에 없는 연산을 만나도 멈추지 않고 CPU 로 돌린다
export PYTORCH_ENABLE_MPS_FALLBACK = 1
export TOKENIZERS_PARALLELISM = false

# 작은 모델부터 잰다(메모리가 16GB 라 한 번에 모델 하나, 한 모델이 process 하나)
MODELS ?= whisper-base whisper-small whisper-large-v3-turbo qwen3-asr-0.6b qwen3-asr-1.7b whisper-large-v3
SETS ?= fleurs_ko fleurs_en zeroth_ko
# 세트마다 앞에서부터 N 개만 잰다(비우면 eval_ids.json 의 전부), 빠른 확인은 N=5
# 세트마다 다르게 주려면 N="fleurs_ko=16 fleurs_en=8 zeroth_ko=12"
N ?=
# 같은 노트북에서 다른 빌드가 도는 일이 많아 우선순위를 낮춘다
NICE = nice -n 10
LORA_MODELS ?= whisper-small whisper-large-v3-turbo qwen3-asr-0.6b
LORA_DEVICES ?= mps cpu
LORA_BATCH ?= 4
LORA_SCOPE ?= all

# 라벨 없는 FL 모의실험(작은 규모가 기본값이다, 큰 규모는 FL_DEVICES=12 FL_UTTS=60 FL_ROUNDS=4 FL_PER_ROUND=6 처럼 준다)
FL_DEVICES ?= 8
FL_UTTS ?= 6
FL_ROUNDS ?= 2
FL_PER_ROUND ?= 4
FL_METHODS ?= a2 a3
FL_EVAL ?= zeroth_ko=40 fleurs_ko=40 fleurs_en=20
FL_TEACHER ?= whisper-large-v3-turbo
FL_LR ?= 1e-4
FL_TAG ?=

.PHONY: install fmt lint test eval-ids prefetch cer score compare lora fl-data fl-label fl-sim

install:
	uv sync

# 시험 세트의 표본 id 목록(data/eval_ids.json)을 만들고 그 음성을 .tmp/hf 에 둔다
eval-ids:
	uv run python -m seamone_research.data

# 형식을 고쳐 쓴다(lint 의 형식 검사가 실패했을 때)
fmt:
	uv run ruff format .

lint:
	uv run ruff check .
	uv run ruff format --check .

test: lint
	uv run pytest

# 가중치를 미리 받는다(측정 중인 모델의 시간을 내려받기에 쓰지 않으려는 것이다)
prefetch:
	$(NICE) uv run python -m seamone_research.prefetch --models "$(MODELS)"

cer:
	@set -e; for model in $(MODELS); do \
		$(NICE) uv run python -m seamone_research.measure --model $$model --sets "$(SETS)" $(if $(N),--n "$(N)"); \
	done
	$(MAKE) score

# 저장된 가설 글로 CER 을 다시 계산한다(모델을 돌리지 않는다)
score:
	uv run python -m seamone_research.score --models "$(MODELS)" --sets "$(SETS)"

# 같은 발화로 모델을 견준다: 세트들을 합친 CER, PAIR="a b" 를 주면 a 에서 b 를 뺀 CER 차이도 낸다
compare:
	uv run python -m seamone_research.compare --models "$(MODELS)" --sets "$(SETS)" $(if $(PAIR),--pair "$(PAIR)")

# Apple GPU 는 한도를 넘으면 시스템이 느려지므로 한도에서 예외로 끝나게 한다(낮은 쪽 비율은 높은 쪽 이하여야 한다)
lora: export PYTORCH_MPS_HIGH_WATERMARK_RATIO = 1.0
lora: export PYTORCH_MPS_LOW_WATERMARK_RATIO = 0.8
lora:
	@for model in $(LORA_MODELS); do \
		for device in $(LORA_DEVICES); do \
			$(NICE) uv run python -m seamone_research.lora_probe --model $$model --device $$device --batch-size $(LORA_BATCH) --scope $(LORA_SCOPE) \
				|| echo "중단됨: $$model $$device"; \
		done; \
	done

# 기기 데이터: Zeroth-Korean train 조각 하나에서 화자 FL_DEVICES 명, 화자마다 발화 FL_UTTS 개(받은 조각은 지운다)
fl-data:
	$(NICE) uv run python -m seamone_research.fl_data --devices $(FL_DEVICES) --utts $(FL_UTTS)

# A2 의 정답: 큰 모델(FL_TEACHER)이 기기 발화에 한 번 달고 내린다
fl-label:
	$(NICE) uv run python -m seamone_research.pseudo_label --model $(FL_TEACHER)

# 모델 하나씩 차례로 돈다(기기 데이터, 큰 모델의 정답, 학생의 FL), A0 은 #2 가 잰 whisper-small 의 가설을 쓴다
fl-sim: export PYTORCH_MPS_HIGH_WATERMARK_RATIO = 1.0
fl-sim: export PYTORCH_MPS_LOW_WATERMARK_RATIO = 0.8
fl-sim:
	$(MAKE) fl-data
	$(if $(findstring a2,$(FL_METHODS)),$(MAKE) fl-label)
	$(NICE) uv run python -m seamone_research.fl_sim --methods "$(FL_METHODS)" --devices $(FL_DEVICES) --utts $(FL_UTTS) --rounds $(FL_ROUNDS) --per-round $(FL_PER_ROUND) --eval "$(FL_EVAL)" --lr $(FL_LR) --tag="$(FL_TAG)"
