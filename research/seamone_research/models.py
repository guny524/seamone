"""시작 모델 후보와 모델마다 정한 실행 설정.

device 와 dtype 은 M1(16GB) 기준이다, bf16 은 쓰지 않는다.
묶음 크기는 4 다(노트북의 메모리가 모자라 메인이 1~4 로 정했다, 2026-11-19).
"""

from dataclasses import dataclass


@dataclass(frozen=True)
class ModelSpec:
    key: str
    repo_id: str
    family: str
    batch_size: int
    device: str = "mps"
    dtype: str = "float16"
    # CPU 에서 학습 시험을 할 때의 dtype(float16 은 CPU 의 역전파에 맞지 않는다)
    cpu_train_dtype: str = "float32"


MODEL_SPECS: dict[str, ModelSpec] = {
    spec.key: spec
    for spec in (
        ModelSpec("whisper-base", "openai/whisper-base", "whisper", batch_size=4),
        ModelSpec("whisper-small", "openai/whisper-small", "whisper", batch_size=4),
        ModelSpec(
            "whisper-large-v3-turbo", "openai/whisper-large-v3-turbo", "whisper", batch_size=4
        ),
        ModelSpec("whisper-large-v3", "openai/whisper-large-v3", "whisper", batch_size=4),
        ModelSpec("qwen3-asr-0.6b", "Qwen/Qwen3-ASR-0.6B", "qwen3-asr", batch_size=4),
        ModelSpec(
            "qwen3-asr-1.7b",
            "Qwen/Qwen3-ASR-1.7B",
            "qwen3-asr",
            batch_size=4,
            cpu_train_dtype="bfloat16",
        ),
    )
}
