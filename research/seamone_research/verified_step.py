"""Public human-labelled training fixture -> one offline Whisper small decoder LoRA update.

This research-only input cannot create an app verification or enter its queue. It proves an
optimizer update, not an improvement in CER or an adapter compatible with GGML inference.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path

import soundfile
import torch
from huggingface_hub import snapshot_download
from peft import LoraConfig, get_peft_model
from transformers import WhisperForConditionalGeneration, WhisperProcessor

from seamone_research.lora_probe import LORA_ALPHA, LORA_RANK, WHISPER_DECODER_TARGETS
from seamone_research.models import MODEL_SPECS
from seamone_research.paths import EVAL_IDS_PATH

MAX_TEXT_BYTES = 32_768
SAMPLE_RATE = 16_000
MAX_SAMPLES = SAMPLE_RATE * 30


def load_fixture(manifest: Path, evaluation: dict) -> tuple[dict, Path]:
    fixture = json.loads(manifest.read_text(encoding="utf-8"))
    required = {"kind", "source", "revision", "split", "utterance_id", "text", "audio", "sha256"}
    if not required <= fixture.keys():
        message = "fixture 필드가 빠졌습니다"
        raise ValueError(message)
    if fixture["kind"] != "public_human_labelled_fixture" or fixture["split"] != "train":
        message = "사람이 정답을 단 공개 train fixture만 이 연구에 쓸 수 있습니다"
        raise ValueError(message)
    excluded = {i for spec in evaluation["sets"].values() for i in spec["ids"]}
    if fixture["utterance_id"] in excluded:
        message = "평가 표본을 학습에 쓸 수 없습니다"
        raise ValueError(message)
    if not fixture["text"].strip() or len(fixture["text"].encode()) > MAX_TEXT_BYTES:
        message = "fixture 정답이 비었거나 너무 깁니다"
        raise ValueError(message)
    audio = (manifest.parent / fixture["audio"]).resolve()
    if audio.parent != manifest.parent.resolve():
        message = "fixture 음성은 manifest와 같은 폴더에 있어야 합니다"
        raise ValueError(message)
    if hashlib.sha256(audio.read_bytes()).hexdigest() != fixture["sha256"]:
        message = "fixture 음성의 SHA-256이 맞지 않습니다"
        raise ValueError(message)
    return fixture, audio


def parameter_digest(parameters) -> str:
    digest = hashlib.sha256()
    for name, parameter in parameters:
        digest.update(name.encode())
        digest.update(parameter.detach().cpu().contiguous().numpy().tobytes())
    return digest.hexdigest()


def optimizer_step(model, batch: dict) -> dict:
    """Hash the frozen base and compare every adapter tensor before and after a real step."""
    frozen = [(n, p) for n, p in model.named_parameters() if not p.requires_grad]
    trainable = [(n, p) for n, p in model.named_parameters() if p.requires_grad]
    if not trainable or any("lora_" not in n for n, _ in trainable):
        message = "LoRA parameter만 학습해야 합니다"
        raise ValueError(message)
    base_before = parameter_digest(frozen)
    before = {n: p.detach().cpu().clone() for n, p in trainable}
    optimizer = torch.optim.AdamW([p for _, p in trainable], lr=1e-4)
    optimizer.zero_grad(set_to_none=True)
    loss = model(**batch).loss
    if not bool(torch.isfinite(loss)):
        message = "손실이 유한하지 않습니다"
        raise ValueError(message)
    loss.backward()
    if any(p.grad is None or not bool(torch.isfinite(p.grad).all()) for _, p in trainable):
        message = "모든 LoRA parameter에 유한한 gradient가 있어야 합니다"
        raise ValueError(message)
    optimizer.step()
    base_after = parameter_digest(frozen)
    delta_squared = sum(
        float((p.detach().cpu() - before[n]).float().square().sum()) for n, p in trainable
    )
    if base_before != base_after or not math.isfinite(delta_squared) or delta_squared <= 0:
        message = "기반 모델은 그대로이고 adapter는 바뀌어야 합니다"
        raise ValueError(message)
    return {
        "loss": float(loss.detach().cpu()),
        "optimizer_steps": 1,
        "adapter_delta_l2": math.sqrt(delta_squared),
        "base_sha256_before": base_before,
        "base_sha256_after": base_after,
        "base_unchanged": True,
        "trainable_parameters": sum(p.numel() for _, p in trainable),
    }


def run(manifest: Path, output: Path, device: str) -> dict:
    evaluation = json.loads(EVAL_IDS_PATH.read_text(encoding="utf-8"))
    fixture, audio_path = load_fixture(manifest, evaluation)
    wave, sample_rate = soundfile.read(audio_path, dtype="float32")
    if sample_rate != SAMPLE_RATE or wave.ndim != 1 or not 0 < len(wave) <= MAX_SAMPLES:
        message = "fixture는 30초 이하 16kHz mono여야 합니다"
        raise ValueError(message)
    spec = MODEL_SPECS["whisper-small"]
    model_path = snapshot_download(spec.repo_id, revision=spec.revision, local_files_only=True)
    processor = WhisperProcessor.from_pretrained(model_path, local_files_only=True)
    processor.tokenizer.set_prefix_tokens(language="korean", task="transcribe")
    base = WhisperForConditionalGeneration.from_pretrained(
        model_path, dtype=torch.float32, local_files_only=True
    ).to(device)
    labels = processor.tokenizer(fixture["text"], return_tensors="pt").input_ids[:, 1:].to(device)
    features = processor.feature_extractor(
        wave, sampling_rate=sample_rate, return_tensors="pt"
    ).input_features.to(device)
    torch.manual_seed(20261120)
    model = get_peft_model(
        base,
        LoraConfig(r=LORA_RANK, lora_alpha=LORA_ALPHA, target_modules=WHISPER_DECODER_TARGETS),
    )
    model.train()
    proof = optimizer_step(model, {"input_features": features, "labels": labels})
    proof.update(
        model=spec.repo_id,
        model_revision=spec.revision,
        fixture_source=fixture["source"],
        fixture_revision=fixture["revision"],
        fixture_split=fixture["split"],
        fixture_sha256=fixture["sha256"],
        quality_claim=False,
        production_activation=False,
        device=device,
    )
    output.mkdir(parents=True, exist_ok=True)
    model.save_pretrained(output / "adapter")
    (output / "proof.json").write_text(json.dumps(proof, indent=2) + "\n", encoding="utf-8")
    return proof


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--device", choices=["cpu", "mps"], default="cpu")
    args = parser.parse_args()
    print(json.dumps(run(args.fixture, args.output, args.device), indent=2))


if __name__ == "__main__":
    main()
