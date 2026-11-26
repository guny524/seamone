"""시작 모델 후보와 모델마다 정한 실행 설정.

device 와 dtype 은 M1(16GB) 기준이다, bf16 은 쓰지 않는다.
묶음 크기는 4 다(노트북의 메모리가 모자라 메인이 1~4 로 정했다, 2026-11-19).
"""

import re
from dataclasses import dataclass

from huggingface_hub import snapshot_download


@dataclass(frozen=True)
class ModelSpec:
    key: str
    repo_id: str
    family: str
    revision: str
    batch_size: int
    device: str = "mps"
    dtype: str = "float16"
    # CPU 에서 학습 시험을 할 때의 dtype(float16 은 CPU 의 역전파에 맞지 않는다)
    cpu_train_dtype: str = "float32"

    def __post_init__(self) -> None:
        revision = self.revision
        if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
            message = "model revision must be a full lowercase 40-character commit SHA"
            raise ValueError(message)


MODEL_SPECS: dict[str, ModelSpec] = {
    spec.key: spec
    for spec in (
        ModelSpec(
            "whisper-base",
            "openai/whisper-base",
            "whisper",
            revision="e37978b90ca9030d5170a5c07aadb050351a65bb",
            batch_size=4,
        ),
        ModelSpec(
            "whisper-small",
            "openai/whisper-small",
            "whisper",
            revision="973afd24965f72e36ca33b3055d56a652f456b4d",
            batch_size=4,
        ),
        ModelSpec(
            "whisper-large-v3-turbo",
            "openai/whisper-large-v3-turbo",
            "whisper",
            revision="41f01f3fe87f28c78e2fbf8b568835947dd65ed9",
            batch_size=4,
        ),
        ModelSpec(
            "whisper-large-v3",
            "openai/whisper-large-v3",
            "whisper",
            revision="06f233fe06e710322aca913c1bc4249a0d71fce1",
            batch_size=4,
        ),
        ModelSpec(
            "qwen3-asr-0.6b",
            "Qwen/Qwen3-ASR-0.6B",
            "qwen3-asr",
            revision="5eb144179a02acc5e5ba31e748d22b0cf3e303b0",
            batch_size=4,
        ),
        ModelSpec(
            "qwen3-asr-1.7b",
            "Qwen/Qwen3-ASR-1.7B",
            "qwen3-asr",
            revision="7278e1e70fe206f11671096ffdd38061171dd6e5",
            batch_size=4,
            cpu_train_dtype="bfloat16",
        ),
    )
}


# safetensors 와 설정 파일만 받는다(.bin 사본과 fp32 사본은 받지 않는다)
ALLOW_PATTERNS = ["*.json", "*.txt", "model.safetensors", "model-*.safetensors"]


def resolve_model_path(spec: ModelSpec) -> str:
    """spec 의 커밋(revision)으로 받은 snapshot 폴더, 모든 로더는 이 경로로 모델을 불러온다."""
    return snapshot_download(spec.repo_id, revision=spec.revision, allow_patterns=ALLOW_PATTERNS)
