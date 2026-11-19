"""메모리를 재고 모델을 내린 뒤 메모리를 돌려준다(노트북 메모리가 모자라 모델은 하나씩 올린다)."""

import gc
import resource

import torch

BYTES_PER_MIB = 1024 * 1024


def release_memory() -> None:
    gc.collect()
    if torch.backends.mps.is_available():
        torch.mps.empty_cache()


def peak_process_mib() -> float:
    # macOS 의 ru_maxrss 는 byte 단위다
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / BYTES_PER_MIB


def device_allocated_mib(device: str) -> float:
    if device == "mps":
        return torch.mps.driver_allocated_memory() / BYTES_PER_MIB
    return 0.0
