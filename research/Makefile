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

.PHONY: install fmt lint test test-offline test-in-network-namespace eval-ids prefetch cer score compare lora training-fixture verified-step

install:
	uv sync

# 시험 세트의 표본 id 목록(data/eval_ids.json)을 만들고 그 음성을 .tmp/hf 에 둔다
eval-ids:
	uv run python -m seamone_research.data

lint:
	uv run ruff check .
	uv run ruff format --check .

test: lint
	uv run pytest

test-offline:
	sudo -n /usr/bin/unshare --net -- /usr/bin/setpriv \
		--reuid="$$(id -u)" --regid="$$(id -g)" --clear-groups --no-new-privs \
		/usr/bin/env -i HOME="$$HOME" PATH="$$PATH" \
		$(MAKE) test-in-network-namespace

test-in-network-namespace:
	@test "$$(id -u)" -ne 0 || { echo "Research tests must run as an unprivileged user"; exit 1; }
	@interfaces="$$(python3 -c 'import socket; print(*(name for _, name in socket.if_nameindex()))')"; \
		echo "Research test network interfaces: $$interfaces"; \
		test "$$interfaces" = lo
	UV_OFFLINE=1 UV_NO_SYNC=1 HF_HUB_OFFLINE=1 $(MAKE) test

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

fmt:
	uv run ruff format .

FIXTURE ?= ../.tmp/research/training-fixture/fixture.json
PROOF_DIR ?= ../.tmp/research/verified-step
PROOF_DEVICE ?= cpu
training-fixture:
	$(NICE) uv run python -m seamone_research.training_fixture --output "$(dir $(FIXTURE))"

verified-step:
	$(NICE) uv run python -m seamone_research.verified_step --fixture "$(FIXTURE)" --output "$(PROOF_DIR)" --device $(PROOF_DEVICE)
