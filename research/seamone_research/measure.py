"""모델 하나가 시험 세트를 받아쓰고 가설 글을 .tmp/research/hyp/<모델>/<세트>.jsonl 에 쓴다.

- 한 줄의 꼴과 읽는 법은 hypotheses.py, eval_ids.json 의 순서대로 쌓는다
- 이미 쓴 id 는 건너뛰어 멈췄다 다시 돌려도 이어 쓴다(끊긴 끝 줄은 버린다)
- 가설 글 옆의 <세트>.config.json 에 측정 설정(모델과 데이터의 revision, 정규화 판)을 쓴다
  이어 쓸 때 대조해 다르면 멈춘다(다른 판의 결과가 한 모델의 CER 로 합쳐지지 않게 한다)
- 채점은 이 파일을 읽는 score.py 가 따로 한다
- 걸린 시간은 같은 폴더의 timing.json 에 쌓는다(다른 worker 의 빌드와 겹치므로 참고값이다)
"""

import argparse
import hashlib
import json
import time
from collections.abc import Iterator, Sequence
from pathlib import Path

import numpy as np
import torch

from seamone_research.cer import NORMALIZATION_VERSION
from seamone_research.data import (
    EVAL_SETS,
    Utterance,
    _write_json_atomically,
    load_eval_ids,
    load_eval_set,
    parse_set_limits,
    prepare_audio,
    read_audio,
)
from seamone_research.hypotheses import encode_row, repair_hypotheses
from seamone_research.memory import device_allocated_mib, peak_process_mib
from seamone_research.models import MODEL_SPECS, ModelSpec
from seamone_research.paths import hypothesis_dir, hypothesis_path
from seamone_research.transcribe import Transcriber, build_transcriber


class LazyTranscriber:
    """처음 받아쓸 때 모델을 불러온다, 할 일이 없으면 모델을 불러오지 않는다."""

    def __init__(self, model: str, spec: ModelSpec, device: str, dtype: str) -> None:
        self._model = model
        self._spec = spec
        self._device = device
        self._dtype = dtype
        self._inner: Transcriber | None = None
        self.load_seconds = 0.0

    def transcribe(self, audio: Sequence[np.ndarray], language: str) -> list[str]:
        if self._inner is None:
            started = time.perf_counter()
            self._inner = build_transcriber(self._spec, self._device, self._dtype)
            self.load_seconds = round(time.perf_counter() - started, 1)
            print(
                f"{self._model}: 불러오기 {self.load_seconds}초(내려받기 포함), "
                f"device={self._device}, dtype={self._dtype}",
                flush=True,
            )
        return self._inner.transcribe(audio, language)

    def close(self) -> None:
        if self._inner is not None:
            self._inner.close()
            self._inner = None


def batched(items: Sequence[Utterance], size: int) -> Iterator[Sequence[Utterance]]:
    for start in range(0, len(items), size):
        yield items[start : start + size]


def append_timing(model: str, record: dict) -> None:
    path = hypothesis_dir(model) / "timing.json"
    runs = json.loads(path.read_text(encoding="utf-8")) if path.exists() else []
    runs.append(record)
    path.write_text(json.dumps(runs, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


def ids_fingerprint(ids: Sequence[str]) -> str:
    """세트의 전체 표본 id 목록(순서 포함)의 해시, 몇 개를 잴지(limit)와 상관없다."""
    return hashlib.sha256("\n".join(ids).encode("utf-8")).hexdigest()


def measurement_config(
    model: str, spec: ModelSpec, set_name: str, device: str, dtype: str, ids: Sequence[str]
) -> dict:
    return {
        "model": model,
        "repo": spec.repo_id,
        "revision": spec.revision,
        "set": set_name,
        "data_revision": EVAL_SETS[set_name].revision,
        "ids_count": len(ids),
        "ids_sha256": ids_fingerprint(ids),
        "normalization": NORMALIZATION_VERSION,
        "device": device,
        "dtype": dtype,
        "decoding": "greedy, 언어 지정, 글 맥락 없음",
    }


def _read_config(path: Path) -> dict | None:
    """저장된 설정, 파일이 없으면 None, 해석되지 않으면 위치를 적은 ValueError 다."""
    if not path.exists():
        return None
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except ValueError as error:  # JSONDecodeError 와 UnicodeDecodeError 모두 ValueError 다
        message = f"{path} 의 측정 설정을 해석하지 못한다({type(error).__name__}: {error})"
        raise ValueError(message) from error
    if not isinstance(value, dict):
        message = f"{path} 의 측정 설정이 객체가 아니다"
        raise ValueError(message)
    return value


def ensure_same_config(path: Path, config: dict, has_rows: bool) -> None:
    """저장된 측정 설정이 지금과 다르면 멈추고, 설정이 없거나 깨졌는데 가설이 없으면 새로 쓴다.

    가설이 있는데 설정이 없거나 깨졌으면 어느 판의 결과인지 알 수 없어 멈춘다.
    """
    try:
        saved = _read_config(path)
    except ValueError:
        if has_rows:
            raise
        saved = None
    if saved is None:
        if has_rows:
            message = (
                f"가설 글은 있는데 측정 설정 파일 {path} 이 없어 어느 판의 결과인지 알 수 없다"
            )
            raise ValueError(message)
        path.parent.mkdir(parents=True, exist_ok=True)
        _write_json_atomically(path, config)
        return
    keys = set(saved) | set(config)
    different = sorted(k for k in keys if saved.get(k) != config.get(k))
    if different:
        message = f"{path} 의 측정 설정이 지금과 다르다: {different}, 이어 쓰지 않는다"
        raise ValueError(message)


def ensure_prefix_of_current_order(path: Path, existing: Sequence[str], ids: Sequence[str]) -> None:
    """쌓인 가설의 id 가 지금 표본 순서의 앞부분과 같아야 이어 쓴다(N 을 늘리는 재개는 통과한다)."""
    if list(existing) != list(ids[: len(existing)]):
        message = f"{path} 에 쌓인 가설의 id 가 지금 표본 순서의 앞부분과 다르다, 이어 쓰지 않는다"
        raise ValueError(message)


def measure_set(
    model: str,
    set_name: str,
    transcriber: Transcriber,
    spec: ModelSpec,
    limit: int | None,
    device: str,
    dtype: str,
) -> dict | None:
    todo_all = load_eval_set(set_name, limit)
    ids = load_eval_ids()["sets"][set_name]["ids"]
    path = hypothesis_path(model, set_name)
    rows = repair_hypotheses(path)
    config = measurement_config(model, spec, set_name, device, dtype, ids)
    ensure_same_config(path.with_suffix(".config.json"), config, bool(rows))
    ensure_prefix_of_current_order(path, [row["id"] for row in rows], ids)
    done = {row["id"] for row in rows}
    todo = [u for u in todo_all if u.id not in done]
    if not todo:
        return None
    language = EVAL_SETS[set_name].language
    path.parent.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    peak_device = 0.0
    finished = len(done)
    with path.open("a", encoding="utf-8") as out:
        for batch in batched(todo, spec.batch_size):
            texts = transcriber.transcribe([read_audio(u) for u in batch], language)
            for utterance, text in zip(batch, texts, strict=True):
                out.write(encode_row(utterance.id, utterance.reference, text, utterance.seconds))
            out.flush()
            finished += len(batch)
            peak_device = max(peak_device, device_allocated_mib(device))
            elapsed = time.perf_counter() - started
            print(f"  {set_name} {finished}/{len(todo_all)} ({elapsed:.0f}초)", flush=True)
    return {
        "set": set_name,
        "n": len(todo),
        "audio_seconds": round(sum(u.seconds for u in todo), 1),
        "wall_seconds": round(time.perf_counter() - started, 1),
        "device_mib_peak": round(peak_device),
        "process_rss_mib_peak": round(peak_process_mib()),
    }


def measure_model(
    model: str,
    set_names: Sequence[str],
    limits: dict[str, int | None],
    device: str | None,
    dtype: str | None,
) -> None:
    spec = MODEL_SPECS[model]
    device = device or spec.device
    dtype = dtype or spec.dtype
    for set_name in set_names:
        prepare_audio(set_name)
    transcriber = LazyTranscriber(model, spec, device, dtype)
    try:
        for set_name in set_names:
            record = measure_set(
                model, set_name, transcriber, spec, limits[set_name], device, dtype
            )
            if record is None:
                print(f"{model} {set_name}: 이미 다 했다")
                continue
            record |= {"model": model, "device": device, "dtype": dtype}
            record |= {"batch_size": spec.batch_size, "load_seconds": transcriber.load_seconds}
            record |= {"torch": torch.__version__}
            append_timing(model, record)
            print(f"{model} {set_name}: {record}")
    finally:
        transcriber.close()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, choices=sorted(MODEL_SPECS))
    parser.add_argument("--sets", default=" ".join(EVAL_SETS))
    parser.add_argument("--n", default=None, help="세트마다 앞에서부터 n 개, 예: 16, fleurs_ko=16")
    parser.add_argument("--device", default=None)
    parser.add_argument("--dtype", default=None)
    args = parser.parse_args()
    set_names = args.sets.split()
    unknown = [name for name in set_names if name not in EVAL_SETS]
    if unknown:
        parser.error(f"모르는 세트: {unknown}")
    limits = parse_set_limits(args.n, set_names)
    measure_model(args.model, set_names, limits, args.device, args.dtype)


if __name__ == "__main__":
    main()
