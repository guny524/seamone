"""LoRA 를 붙인 모델이 음성 4개 한 묶음으로 순전파와 역전파를 도는지 재 본다.

- 손실은 정답 글에 걸고, LoRA 가중치로 기울기가 유한하게 흐르는지 확인한다
- 걸음을 두 번 돌려 첫 걸음(커널 준비 포함)과 두 번째 걸음의 시간을 따로 적는다
- LoRA 는 q_proj 와 v_proj 에 붙인다, 범위(scope)는 all(음성 인코더와 글 디코더 모두) 또는
  decoder(글 디코더만, 음성 인코더는 그래프를 만들지 않아 메모리가 훨씬 적다)
- Qwen3-ASR 은 학습용 입력을 만드는 API 가 없어 추론과 같은 프롬프트 뒤에 정답 글을 붙여 만든다
  (프롬프트 토큰은 손실에서 가린다, 역전파가 도는지와 비용을 보는 시험이라 이 정도로 충분하다)
- 결과는 .tmp/research/lora/<모델>-<device>-b<묶음 크기>.json(decoder 범위는 끝에 -decoder)
"""

import argparse
import json
import math
import time
import traceback
from collections.abc import Sequence
from dataclasses import asdict, dataclass

import torch
from peft import LoraConfig, get_peft_model
from transformers import WhisperForConditionalGeneration, WhisperProcessor

from seamone_research.data import SAMPLE_RATE, Utterance, load_eval_set, prepare_audio, read_audio
from seamone_research.memory import device_allocated_mib, peak_process_mib, release_memory
from seamone_research.models import MODEL_SPECS, ModelSpec, resolve_model_path
from seamone_research.paths import RUN_DIR

LORA_RANK = 8
LORA_ALPHA = 16
LORA_ALL_TARGETS = ["q_proj", "v_proj"]
WHISPER_DECODER_TARGETS = r"model\.decoder\.layers\.\d+\.(self_attn|encoder_attn)\.(q_proj|v_proj)"
QWEN_DECODER_TARGETS = r"model\.layers\.\d+\.self_attn\.(q_proj|v_proj)"
BATCH_SIZE = 4
STEPS = 2
PROBE_SET = "fleurs_ko"
QWEN_END_OF_TURN = "<|im_end|>"
IGNORE_INDEX = -100


def lora_targets(family: str, scope: str) -> list[str] | str:
    """peft 의 target_modules: 이름 목록(범위 all) 또는 모듈 경로 정규식(범위 decoder)."""
    if scope == "all":
        return LORA_ALL_TARGETS
    if scope != "decoder":
        message = f"모르는 범위: {scope}"
        raise ValueError(message)
    return WHISPER_DECODER_TARGETS if family == "whisper" else QWEN_DECODER_TARGETS


@dataclass
class ProbeResult:
    model: str
    device: str
    dtype: str
    ok: bool
    scope: str = "all"
    batch_size: int = BATCH_SIZE
    loss: float | None = None
    forward_seconds: list[float] | None = None
    backward_seconds: list[float] | None = None
    trainable_params: int | None = None
    total_params: int | None = None
    grads_finite: bool | None = None
    params_without_grad: int | None = None
    losses_finite: bool | None = None
    device_mib: float | None = None
    process_rss_mib_peak: float | None = None
    error: str | None = None


@dataclass(frozen=True)
class ProbeJudgement:
    ok: bool
    grads_finite: bool
    params_without_grad: int
    losses_finite: bool


def judge_probe(grads: Sequence[torch.Tensor | None], losses: Sequence[float]) -> ProbeJudgement:
    """모든 학습 가능한 parameter 에 유한한 기울기가 있고 모든 걸음의 손실이 유한할 때만 성공이다.

    grads 는 학습 가능한 parameter 마다의 기울기이고, 손실과 이어지지 않으면 None 이다.
    """
    missing = sum(grad is None for grad in grads)
    finite = bool(grads) and all(bool(torch.isfinite(g).all()) for g in grads if g is not None)
    losses_finite = bool(losses) and all(math.isfinite(loss) for loss in losses)
    return ProbeJudgement(missing == 0 and finite and losses_finite, finite, missing, losses_finite)


def synchronize(device: str) -> None:
    if device == "mps":
        torch.mps.synchronize()


def whisper_model_and_batch(
    spec: ModelSpec, device: str, dtype: torch.dtype, utterances: Sequence[Utterance]
) -> tuple[torch.nn.Module, dict]:
    model_path = resolve_model_path(spec)
    processor = WhisperProcessor.from_pretrained(model_path)
    processor.tokenizer.set_prefix_tokens(language="korean", task="transcribe")
    model = WhisperForConditionalGeneration.from_pretrained(model_path, dtype=dtype).to(device)
    features = processor.feature_extractor(
        [read_audio(u) for u in utterances], sampling_rate=SAMPLE_RATE, return_tensors="pt"
    ).input_features
    references = [u.reference for u in utterances]
    tokens = processor.tokenizer(references, return_tensors="pt", padding=True)
    labels = tokens.input_ids.masked_fill(tokens.attention_mask.eq(0), IGNORE_INDEX)
    if (labels[:, 0] == model.config.decoder_start_token_id).all():
        labels = labels[:, 1:]
    return model, {"input_features": features.to(device, dtype), "labels": labels.to(device)}


def qwen_prompt(processor) -> str:
    messages = [
        {"role": "system", "content": ""},
        {"role": "user", "content": [{"type": "audio", "audio": ""}]},
    ]
    template = processor.apply_chat_template(messages, add_generation_prompt=True, tokenize=False)
    return template + "language Korean<asr_text>"


def qwen_labels(processor, prompt: str, waves: Sequence, inputs) -> torch.Tensor:
    labels = inputs["input_ids"].masked_fill(inputs["attention_mask"].eq(0), IGNORE_INDEX)
    left_padded = processor.tokenizer.padding_side == "left"
    for row, wave in enumerate(waves):
        prompt_inputs = processor(text=[prompt], audio=[wave], return_tensors="pt")
        prompt_length = prompt_inputs["input_ids"].shape[1]
        first = int(inputs["attention_mask"][row].eq(0).sum()) if left_padded else 0
        labels[row, first : first + prompt_length] = IGNORE_INDEX
    return labels


def qwen_model_and_batch(
    spec: ModelSpec, device: str, dtype: torch.dtype, utterances: Sequence[Utterance]
) -> tuple[torch.nn.Module, dict]:
    from qwen_asr import Qwen3ASRModel

    asr = Qwen3ASRModel.from_pretrained(resolve_model_path(spec), dtype=dtype, device_map=device)
    processor = asr.processor
    prompt = qwen_prompt(processor)
    waves = [read_audio(u) for u in utterances]
    texts = [prompt + u.reference + QWEN_END_OF_TURN for u in utterances]
    inputs = processor(text=texts, audio=waves, return_tensors="pt", padding=True)
    labels = qwen_labels(processor, prompt, waves, inputs)
    batch = dict(inputs.to(device).to(dtype))
    batch["labels"] = labels.to(device)
    return asr.model.thinker, batch


def train_step(model: torch.nn.Module, batch: dict, device: str) -> tuple[float, float, float]:
    model.zero_grad(set_to_none=True)
    synchronize(device)
    started = time.perf_counter()
    loss = model(**batch).loss
    synchronize(device)
    forward_done = time.perf_counter()
    loss.backward()
    synchronize(device)
    return loss.item(), forward_done - started, time.perf_counter() - forward_done


def run_probe(
    model_key: str,
    device: str,
    dtype: str,
    batch_size: int = BATCH_SIZE,
    steps: int = STEPS,
    scope: str = "all",
) -> ProbeResult:
    spec = MODEL_SPECS[model_key]
    result = ProbeResult(model_key, device, dtype, ok=False, scope=scope, batch_size=batch_size)
    try:
        prepare_audio(PROBE_SET)
        utterances = load_eval_set(PROBE_SET, batch_size)
        torch_dtype = getattr(torch, dtype)
        build = whisper_model_and_batch if spec.family == "whisper" else qwen_model_and_batch
        base, batch = build(spec, device, torch_dtype, utterances)
        targets = lora_targets(spec.family, scope)
        config = LoraConfig(r=LORA_RANK, lora_alpha=LORA_ALPHA, target_modules=targets)
        model = get_peft_model(base, config)
        model.train()
        result.trainable_params, result.total_params = model.get_nb_trainable_parameters()
        taken = [train_step(model, batch, device) for _ in range(steps)]
        result.loss = taken[-1][0]
        result.forward_seconds = [round(step[1], 2) for step in taken]
        result.backward_seconds = [round(step[2], 2) for step in taken]
        grads = [p.grad for p in model.parameters() if p.requires_grad]
        judgement = judge_probe(grads, [step[0] for step in taken])
        result.grads_finite = judgement.grads_finite
        result.params_without_grad = judgement.params_without_grad
        result.losses_finite = judgement.losses_finite
        result.device_mib = round(device_allocated_mib(device))
        result.process_rss_mib_peak = round(peak_process_mib())
        result.ok = judgement.ok
    except Exception as error:
        result.error = f"{type(error).__name__}: {error}"[:400]
        traceback.print_exc()
    finally:
        model = batch = base = None
        release_memory()
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, choices=sorted(MODEL_SPECS))
    parser.add_argument("--device", required=True, choices=["mps", "cpu"])
    parser.add_argument("--batch-size", type=int, default=BATCH_SIZE)
    parser.add_argument("--steps", type=int, default=STEPS)
    parser.add_argument("--scope", choices=["all", "decoder"], default="all")
    args = parser.parse_args()
    spec = MODEL_SPECS[args.model]
    dtype = spec.dtype if args.device == "mps" else spec.cpu_train_dtype
    result = run_probe(args.model, args.device, dtype, args.batch_size, args.steps, args.scope)
    out_dir = RUN_DIR / "lora"
    out_dir.mkdir(parents=True, exist_ok=True)
    text = json.dumps(asdict(result), ensure_ascii=False, indent=1)
    suffix = "" if args.scope == "all" else f"-{args.scope}"
    name = f"{args.model}-{args.device}-b{args.batch_size}{suffix}.json"
    (out_dir / name).write_text(text + "\n", encoding="utf-8")
    print(text)


if __name__ == "__main__":
    main()
