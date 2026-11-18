"""학생 모델(whisper-small + 글 디코더 LoRA): 기기 하나의 학습, 가중치 주고받기, 받아쓰기, 믿음도.

- 기기마다 전역 LoRA 가중치에서 시작하므로 기반 모델은 한 번만 올리고 LoRA 가중치만 바꿔 끼운다
- 학습 입력의 SpecAugment 와 소음은 A3 에만 쓴다(augment_rng 를 주면 켠다)
"""

import time
from collections.abc import Mapping, Sequence

import numpy as np
import torch
from peft import LoraConfig, get_peft_model
from transformers import WhisperForConditionalGeneration, WhisperProcessor

from seamone_research.data import SAMPLE_RATE, Utterance, read_audio
from seamone_research.lora_probe import (
    IGNORE_INDEX,
    LORA_ALPHA,
    LORA_RANK,
    WHISPER_DECODER_TARGETS,
)
from seamone_research.memory import release_memory
from seamone_research.models import MODEL_SPECS, resolve_model_path
from seamone_research.pseudo_label import mean_token_probability
from seamone_research.transcribe import WHISPER_MAX_NEW_TOKENS

FRAMES_PER_SECOND = 100
PREFIX_TOKENS = 3  # <|ko|> <|transcribe|> <|notimestamps|>, 강제로 붙는 토큰이라 믿음도에서 뺀다
NOISE_SNR_DB = 20.0
FREQ_MASKS, FREQ_WIDTH = 2, 10
TIME_MASKS, TIME_WIDTH = 2, 50


def add_noise(
    wave: np.ndarray, rng: np.random.Generator, snr_db: float = NOISE_SNR_DB
) -> np.ndarray:
    """흰 소음을 신호 대 잡음비 snr_db 로 섞는다."""
    power = max(float(np.mean(wave**2)), 1e-8)
    noise = rng.standard_normal(len(wave)).astype(np.float32)
    return (wave + noise * np.sqrt(power / 10 ** (snr_db / 10))).astype(np.float32)


def spec_augment(
    features: torch.Tensor, rng: np.random.Generator, valid_frames: Sequence[int]
) -> torch.Tensor:
    """로그 멜 특징(묶음, 멜, 프레임)의 주파수 띠와 시간 띠를 평균값으로 가린다(입력은 그대로)."""
    out = features.clone()
    for row, frames in zip(out, valid_frames, strict=True):
        fill = row.mean()
        for _ in range(FREQ_MASKS):
            width = int(rng.integers(0, FREQ_WIDTH + 1))
            start = int(rng.integers(0, row.shape[0] - width + 1))
            row[start : start + width, :] = fill
        for _ in range(TIME_MASKS):
            width = int(rng.integers(0, TIME_WIDTH + 1))
            start = int(rng.integers(0, max(1, min(frames, row.shape[1]) - width + 1)))
            row[:, start : start + width] = fill
    return out


class Student:
    def __init__(self, model_key: str, device: str, dtype: str, seed: int) -> None:
        spec = MODEL_SPECS[model_key]
        self.device = device
        self.dtype = getattr(torch, dtype)
        model_path = resolve_model_path(spec)
        self.processor = WhisperProcessor.from_pretrained(model_path)
        self.processor.tokenizer.set_prefix_tokens(language="korean", task="transcribe")
        base = WhisperForConditionalGeneration.from_pretrained(model_path, dtype=self.dtype)
        self.start_token = base.config.decoder_start_token_id
        torch.manual_seed(seed)
        config = LoraConfig(
            r=LORA_RANK, lora_alpha=LORA_ALPHA, target_modules=WHISPER_DECODER_TARGETS
        )
        self.model = get_peft_model(base.to(device), config)
        self.params = {n: p for n, p in self.model.named_parameters() if p.requires_grad}

    def lora_state(self) -> dict[str, torch.Tensor]:
        return {n: p.detach().to("cpu", torch.float32).clone() for n, p in self.params.items()}

    def load_lora_state(self, state: Mapping[str, torch.Tensor]) -> None:
        with torch.no_grad():
            for name, param in self.params.items():
                param.copy_(state[name].to(param.device, param.dtype))

    def _features(self, waves: Sequence[np.ndarray]) -> torch.Tensor:
        extractor = self.processor.feature_extractor
        return extractor(list(waves), sampling_rate=SAMPLE_RATE, return_tensors="pt").input_features

    def _labels(self, texts: Sequence[str]) -> torch.Tensor:
        tokens = self.processor.tokenizer(list(texts), return_tensors="pt", padding=True)
        labels = tokens.input_ids.masked_fill(tokens.attention_mask.eq(0), IGNORE_INDEX)
        if (labels[:, 0] == self.start_token).all():
            labels = labels[:, 1:]
        return labels

    def train_client(
        self,
        utterances: Sequence[Utterance],
        texts: Sequence[str],
        lr: float,
        batch_size: int,
        augment_rng: np.random.Generator | None = None,
    ) -> tuple[float, float]:
        """발화 한 바퀴를 학습한다, (평균 손실, 걸린 초)를 돌려준다."""
        self.model.train()
        optimizer = torch.optim.AdamW(list(self.params.values()), lr=lr)
        losses = []
        started = time.perf_counter()
        for start in range(0, len(utterances), batch_size):
            chunk = utterances[start : start + batch_size]
            waves = [read_audio(u) for u in chunk]
            if augment_rng is not None:
                waves = [add_noise(w, augment_rng) for w in waves]
            features = self._features(waves)
            if augment_rng is not None:
                frames = [int(u.seconds * FRAMES_PER_SECOND) for u in chunk]
                features = spec_augment(features, augment_rng, frames)
            labels = self._labels(texts[start : start + batch_size]).to(self.device)
            optimizer.zero_grad(set_to_none=True)
            loss = self.model(
                input_features=features.to(self.device, self.dtype), labels=labels
            ).loss
            loss.backward()
            optimizer.step()
            losses.append(loss.item())
        if self.device == "mps":
            torch.mps.synchronize()
        elapsed = time.perf_counter() - started
        del optimizer
        release_memory()
        return sum(losses) / len(losses), elapsed

    @torch.inference_mode()
    def transcribe(self, waves: Sequence[np.ndarray], language: str) -> list[str]:
        self.model.eval()
        features = self._features(waves).to(self.device, self.dtype)
        generated = self.model.generate(
            input_features=features,
            language=language,
            task="transcribe",
            num_beams=1,
            do_sample=False,
            max_new_tokens=WHISPER_MAX_NEW_TOKENS,
            return_timestamps=False,
        )
        return [t.strip() for t in self.processor.batch_decode(generated, skip_special_tokens=True)]

    @torch.inference_mode()
    def confidences(self, waves: Sequence[np.ndarray], texts: Sequence[str]) -> list[float]:
        """단 글을 정답으로 놓고 한 번 순전파해 글 토큰(끝 토큰 포함)의 평균 확률을 낸다."""
        self.model.eval()
        features = self._features(waves).to(self.device, self.dtype)
        labels = self._labels(texts).to(self.device)
        logits = self.model(input_features=features, labels=labels).logits.float()
        picked = logits.softmax(-1).gather(-1, labels.clamp(min=0).unsqueeze(-1)).squeeze(-1)
        keep = labels.ne(IGNORE_INDEX)
        keep[:, :PREFIX_TOKENS] = False
        return [mean_token_probability(picked[i][keep[i]].tolist()) for i in range(len(texts))]
