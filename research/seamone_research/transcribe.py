"""후보 모델의 받아쓰기: 음성 목록을 받아 글 목록을 내는 같은 interface(Transcriber) 뒤에 둔다.

- Whisper 계열은 transformers, Qwen3-ASR 은 공식 패키지(qwen-asr)의 transformers backend 를 쓴다
- 언어는 세트의 언어로 지정하고, 풀어 쓰기는 greedy(빔 1, 표집 없음)다
"""

from collections.abc import Sequence
from typing import Protocol

import numpy as np
import torch
from transformers import WhisperForConditionalGeneration, WhisperProcessor

from seamone_research.data import SAMPLE_RATE
from seamone_research.memory import release_memory
from seamone_research.models import ModelSpec

# Whisper 의 디코더 길이 한도 448 에서 시작 토큰 4개와 여유를 뺀 값
WHISPER_MAX_NEW_TOKENS = 440
QWEN_MAX_NEW_TOKENS = 512
QWEN_LANGUAGE_NAMES = {"ko": "Korean", "en": "English"}


class Transcriber(Protocol):
    def transcribe(self, audio: Sequence[np.ndarray], language: str) -> list[str]: ...

    def close(self) -> None: ...


class WhisperTranscriber:
    def __init__(self, repo_id: str, device: str, dtype: torch.dtype) -> None:
        self._device = device
        self._dtype = dtype
        self._processor = WhisperProcessor.from_pretrained(repo_id)
        model = WhisperForConditionalGeneration.from_pretrained(repo_id, dtype=dtype)
        self._model = model.to(device).eval()

    @torch.inference_mode()
    def transcribe(self, audio: Sequence[np.ndarray], language: str) -> list[str]:
        features = self._processor.feature_extractor(
            list(audio), sampling_rate=SAMPLE_RATE, return_tensors="pt"
        )
        generated = self._model.generate(
            features.input_features.to(self._device, self._dtype),
            language=language,
            task="transcribe",
            num_beams=1,
            do_sample=False,
            max_new_tokens=WHISPER_MAX_NEW_TOKENS,
            return_timestamps=False,
        )
        decoded = self._processor.batch_decode(generated, skip_special_tokens=True)
        return [text.strip() for text in decoded]

    def close(self) -> None:
        del self._model
        release_memory()


class QwenAsrTranscriber:
    def __init__(self, repo_id: str, device: str, dtype: torch.dtype, batch_size: int) -> None:
        from qwen_asr import Qwen3ASRModel

        self._model = Qwen3ASRModel.from_pretrained(
            repo_id,
            dtype=dtype,
            device_map=device,
            max_inference_batch_size=batch_size,
            max_new_tokens=QWEN_MAX_NEW_TOKENS,
        )

    @torch.inference_mode()
    def transcribe(self, audio: Sequence[np.ndarray], language: str) -> list[str]:
        results = self._model.transcribe(
            audio=[(wave, SAMPLE_RATE) for wave in audio],
            language=[QWEN_LANGUAGE_NAMES[language]] * len(audio),
        )
        return [result.text.strip() for result in results]

    def close(self) -> None:
        del self._model
        release_memory()


def build_transcriber(spec: ModelSpec, device: str, dtype: str) -> Transcriber:
    torch_dtype = getattr(torch, dtype)
    if spec.family == "whisper":
        return WhisperTranscriber(spec.repo_id, device, torch_dtype)
    return QwenAsrTranscriber(spec.repo_id, device, torch_dtype, spec.batch_size)
