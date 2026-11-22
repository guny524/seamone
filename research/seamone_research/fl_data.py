"""라벨 없는 FL 모의실험의 기기 데이터: Zeroth-Korean train 의 화자 하나를 기기 하나로 삼는다.

- 후보: train 조각 한 파일에서 30초 이하이고 정규화한 정답이 비지 않은 녹음
  시험 세트(test)의 화자는 뺀다
- 기기: 후보가 충분한 화자 가운데 seed 와 화자 id 를 이은 글의 SHA-256 순서로 n 명
  화자마다 같은 방식으로 발화 n 개
- 음성은 .tmp/hf/seamone_fl 에만 두고, 내려받은 train 조각은 wav 를 꺼낸 뒤 지운다
- 뽑은 id 목록은 research/data/fl_client_ids.json 이다
"""

import argparse
import hashlib
import io
import json
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

import pyarrow.parquet as pq
import soundfile
from huggingface_hub import hf_hub_download

from seamone_research.cer import normalize
from seamone_research.data import (
    EVAL_SETS,
    MAX_SECONDS,
    SEED,
    Utterance,
    _save_wav,
    _wav_name,
    remove_inside,
)
from seamone_research.paths import RESEARCH_DIR, hf_home

TRAIN_FILE = "data/train-00000-of-00006.parquet"
FL_IDS_PATH = RESEARCH_DIR / "data" / "fl_client_ids.json"
DEVICES = 8
UTTS_PER_DEVICE = 6
BATCH_ROWS = 128


@dataclass(frozen=True)
class FlCandidate:
    id: str
    speaker: int
    reference: str
    seconds: float


@dataclass(frozen=True)
class Client:
    name: str
    speaker: int
    utterances: list[Utterance]


def fl_audio_dir() -> Path:
    return hf_home() / "seamone_fl"


def fl_raw_cache_dir() -> Path:
    """받은 train 조각의 전용 cache, 공유 hub/ 를 지우지 않고 이 폴더만 지운다."""
    return hf_home() / "fl-raw" / "zeroth_train"


def _rank_key(seed: int, key: str) -> str:
    return hashlib.sha256(f"{seed}:{key}".encode()).hexdigest()


def select_clients(
    candidates: Sequence[FlCandidate],
    n_devices: int,
    utts_per_device: int,
    excluded_speakers: frozenset[int] | set[int] = frozenset(),
    seed: int = SEED,
) -> dict[int, list[str]]:
    """화자 id 에서 발화 id 목록으로, 화자 순서와 발화 순서 모두 SHA-256 순서다."""
    by_speaker: dict[int, list[FlCandidate]] = {}
    for candidate in sorted(candidates, key=lambda c: c.id):
        if candidate.speaker in excluded_speakers or candidate.seconds > MAX_SECONDS:
            continue
        if normalize(candidate.reference):
            by_speaker.setdefault(candidate.speaker, []).append(candidate)
    enough = [s for s, items in by_speaker.items() if len(items) >= utts_per_device]
    if len(enough) < n_devices:
        message = (
            f"발화 {utts_per_device}개 이상인 화자 {len(enough)}명으로 {n_devices}명을 뽑을 수 없다"
        )
        raise ValueError(message)
    speakers = sorted(enough, key=lambda s: _rank_key(seed, f"speaker:{s}"))[:n_devices]
    return {
        speaker: [
            c.id
            for c in sorted(by_speaker[speaker], key=lambda c: _rank_key(seed, c.id))[
                :utts_per_device
            ]
        ]
        for speaker in speakers
    }


def _scan(path: Path) -> list[FlCandidate]:
    candidates = []
    columns = ["id", "speaker_id", "text", "audio"]
    for batch in pq.ParquetFile(path).iter_batches(batch_size=BATCH_ROWS, columns=columns):
        for row in batch.to_pylist():
            seconds = soundfile.info(io.BytesIO(row["audio"]["bytes"])).duration
            candidates.append(FlCandidate(row["id"], int(row["speaker_id"]), row["text"], seconds))
    return candidates


def _extract(path: Path, wanted: set[str]) -> dict[str, bytes]:
    found = {}
    for batch in pq.ParquetFile(path).iter_batches(batch_size=BATCH_ROWS, columns=["id", "audio"]):
        for row in batch.to_pylist():
            if row["id"] in wanted:
                found[row["id"]] = row["audio"]["bytes"]
    return found


def build_clients(n_devices: int = DEVICES, utts_per_device: int = UTTS_PER_DEVICE) -> dict:
    spec = EVAL_SETS["zeroth_ko"]
    fetch = {"repo_type": "dataset", "revision": spec.revision, "cache_dir": fl_raw_cache_dir()}
    test_path = hf_hub_download(spec.repo_id, spec.split_file, **fetch)
    test_speakers = set(
        pq.read_table(test_path, columns=["speaker_id"]).column("speaker_id").to_pylist()
    )
    train_path = Path(hf_hub_download(spec.repo_id, TRAIN_FILE, **fetch))
    candidates = _scan(train_path)
    chosen = select_clients(candidates, n_devices, utts_per_device, test_speakers)
    by_id = {c.id: c for c in candidates}
    raw = _extract(train_path, {i for ids in chosen.values() for i in ids})
    out_dir = fl_audio_dir()
    out_dir.mkdir(parents=True, exist_ok=True)
    references, devices = {}, []
    for index, (speaker, ids) in enumerate(chosen.items()):
        total = 0.0
        for utterance_id in ids:
            seconds = _save_wav(out_dir / _wav_name(utterance_id), raw[utterance_id])
            references[utterance_id] = {
                "reference": by_id[utterance_id].reference,
                "seconds": round(seconds, 3),
                "speaker": speaker,
            }
            total += seconds
        devices.append(
            {"name": f"device-{index}", "speaker": speaker, "ids": ids, "seconds": round(total, 1)}
        )
    (out_dir / "references.json").write_text(
        json.dumps(references, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    result = {
        "seed": SEED,
        "source": spec.repo_id,
        "revision": spec.revision,
        "train_file": TRAIN_FILE,
        "train_candidates": len(candidates),
        "train_speakers": len({c.speaker for c in candidates}),
        "test_speakers": sorted(test_speakers),
        "overlap_with_test_speakers": len({d["speaker"] for d in devices} & test_speakers),
        "devices": devices,
    }
    FL_IDS_PATH.write_text(
        json.dumps(result, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    remove_inside(hf_home() / "fl-raw", fl_raw_cache_dir())
    return result


def load_clients(n_devices: int | None = None, utts_per_device: int | None = None) -> list[Client]:
    info = json.loads(FL_IDS_PATH.read_text(encoding="utf-8"))
    references = json.loads((fl_audio_dir() / "references.json").read_text(encoding="utf-8"))
    clients = []
    for device in info["devices"][:n_devices]:
        utterances = [
            Utterance(
                id=utterance_id,
                reference=references[utterance_id]["reference"],
                audio_path=fl_audio_dir() / _wav_name(utterance_id),
                seconds=references[utterance_id]["seconds"],
            )
            for utterance_id in device["ids"][:utts_per_device]
        ]
        clients.append(Client(device["name"], device["speaker"], utterances))
    return clients


def clients_ready(n_devices: int, utts_per_device: int) -> bool:
    """뽑은 id 목록과 wav 가 이 규모로 이미 있으면 참이다."""
    if not FL_IDS_PATH.exists():
        return False
    devices = json.loads(FL_IDS_PATH.read_text(encoding="utf-8"))["devices"]
    sizes_match = len(devices) == n_devices and all(
        len(d["ids"]) == utts_per_device for d in devices
    )
    return sizes_match and all(
        (fl_audio_dir() / _wav_name(i)).exists() for d in devices for i in d["ids"]
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--devices", type=int, default=DEVICES)
    parser.add_argument("--utts", type=int, default=UTTS_PER_DEVICE)
    args = parser.parse_args()
    if clients_ready(args.devices, args.utts):
        print("기기 데이터가 이미 있다, 다시 받지 않는다")
        return
    result = build_clients(args.devices, args.utts)
    print(f"train 후보 {result['train_candidates']}개, 화자 {result['train_speakers']}명")
    print(f"시험 세트(test) 화자와 겹치는 기기 화자: {result['overlap_with_test_speakers']}명")
    for device in result["devices"]:
        count = len(device["ids"])
        print(f"{device['name']}: 화자 {device['speaker']}, 발화 {count}개, {device['seconds']}초")


if __name__ == "__main__":
    main()
