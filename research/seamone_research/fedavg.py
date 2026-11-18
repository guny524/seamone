"""FL 로 합치기: 서버가 기기마다 보낸 LoRA 가중치를 발화 수로 가중 평균한다(FedAvg)."""

import random
from collections.abc import Mapping, Sequence

import torch


def fedavg(updates: Sequence[tuple[Mapping[str, torch.Tensor], int]]) -> dict[str, torch.Tensor]:
    """(가중치 상태, 발화 수) 목록을 발화 수로 가중 평균한다, 결과는 float32 다."""
    if not updates:
        message = "합칠 업데이트가 없다"
        raise ValueError(message)
    total = sum(count for _, count in updates)
    if total <= 0 or any(count < 0 for _, count in updates):
        message = "발화 수의 합이 0 이하이거나 음수가 있다"
        raise ValueError(message)
    keys = set(updates[0][0])
    if any(set(state) != keys for state, _ in updates):
        message = "기기마다 LoRA 가중치 이름이 다르다"
        raise ValueError(message)
    return {
        key: torch.stack([state[key].float() * count for state, count in updates]).sum(0) / total
        for key in updates[0][0]
    }


def state_megabytes(state: Mapping[str, torch.Tensor]) -> float:
    """업데이트 하나를 기기에서 서버로 보낼 때의 크기(MB, 1MB 는 10 의 6 제곱 바이트)."""
    return sum(t.numel() * t.element_size() for t in state.values()) / 1e6


def sample_devices(n_devices: int, per_round: int, rounds: int, seed: int) -> list[list[int]]:
    """라운드마다 기기 per_round 개를 중복 없이 뽑는다, seed 가 같으면 방법이 달라도 같은 기기다."""
    if per_round > n_devices:
        message = f"기기 {n_devices}개에서 {per_round}개를 뽑을 수 없다"
        raise ValueError(message)
    rng = random.Random(seed)
    return [sorted(rng.sample(range(n_devices), per_round)) for _ in range(rounds)]
