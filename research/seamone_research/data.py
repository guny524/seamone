"""시험 세트: 고정한 방법으로 뽑은 표본의 id 목록과 16kHz mono 음성.

- 후보: 세트의 test split 에서 30초 이하(Whisper 의 입력 창)이고 정규화한 정답이 비지 않은 녹음
- 같은 문장을 여러 사람이 읽은 녹음(FLEURS)은 file_name 이 가장 앞선 하나만 후보로 둔다
- 뽑기: seed 와 id 를 이은 글의 SHA-256 순서로 줄 세워 앞에서 count 개를 뽑는다
  이 순서가 eval_ids.json 의 순서다(앞에서부터 잘라 쓰면 모든 모델이 같은 표본의 앞부분을 쓴다)
- 음성은 .tmp/hf 에만 둔다, test split 한 파일만 받아 뽑힌 녹음만 꺼내고 원본은 지운다
"""

import hashlib
import io
import json
import shutil
import tarfile
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

import librosa
import numpy as np
import pyarrow.parquet as pq
import soundfile
from huggingface_hub import hf_hub_download

from seamone_research.cer import normalize
from seamone_research.paths import EVAL_IDS_PATH, eval_audio_dir, hf_home

SEED = 20261119
SAMPLE_RATE = 16_000
MAX_SECONDS = 30.0
FLEURS_REVISION = "70bb2e84b976b7e960aa89f1c648e09c59f894dd"
ZEROTH_REVISION = "1fe937899f828af822293d05e086200946088bdf"


@dataclass(frozen=True)
class Candidate:
    id: str
    sentence_key: str
    reference: str
    seconds: float


@dataclass(frozen=True)
class Utterance:
    id: str
    reference: str
    audio_path: Path
    seconds: float


@dataclass(frozen=True)
class EvalSetSpec:
    name: str
    language: str
    repo_id: str
    revision: str
    count: int
    split_file: str
    audio_file: str | None


EVAL_SETS: dict[str, EvalSetSpec] = {
    "fleurs_ko": EvalSetSpec(
        name="fleurs_ko",
        language="ko",
        repo_id="google/fleurs",
        revision=FLEURS_REVISION,
        count=200,
        split_file="data/ko_kr/test.tsv",
        audio_file="data/ko_kr/audio/test.tar.gz",
    ),
    "fleurs_en": EvalSetSpec(
        name="fleurs_en",
        language="en",
        repo_id="google/fleurs",
        revision=FLEURS_REVISION,
        count=100,
        split_file="data/en_us/test.tsv",
        audio_file="data/en_us/audio/test.tar.gz",
    ),
    "zeroth_ko": EvalSetSpec(
        name="zeroth_ko",
        language="ko",
        repo_id="kresnik/zeroth_korean",
        revision=ZEROTH_REVISION,
        count=200,
        split_file="data/test-00000-of-00001.parquet",
        audio_file=None,
    ),
}


def select_ids(
    candidates: Sequence[Candidate],
    count: int,
    seed: int = SEED,
    max_seconds: float = MAX_SECONDS,
) -> list[str]:
    eligible = [c for c in candidates if c.seconds <= max_seconds and normalize(c.reference)]
    one_per_sentence: dict[str, Candidate] = {}
    for candidate in sorted(eligible, key=lambda c: c.id):
        one_per_sentence.setdefault(candidate.sentence_key, candidate)
    if len(one_per_sentence) < count:
        message = f"후보 {len(one_per_sentence)}개로 {count}개를 뽑을 수 없다"
        raise ValueError(message)
    ranked = sorted(one_per_sentence.values(), key=lambda c: _rank_key(seed, c.id))
    return [candidate.id for candidate in ranked[:count]]


def _rank_key(seed: int, candidate_id: str) -> str:
    return hashlib.sha256(f"{seed}:{candidate_id}".encode()).hexdigest()


def _fleurs_candidates(tsv_path: Path) -> list[Candidate]:
    # 머리글 없는 7열: id(문장), file_name, raw_transcription, transcription,
    # 글자 나눔, num_samples, gender
    candidates = []
    for line in tsv_path.read_text(encoding="utf-8").splitlines():
        sentence_id, file_name, raw_text, _, _, num_samples, _ = line.split("\t")
        candidates.append(
            Candidate(
                id=file_name,
                sentence_key=sentence_id,
                reference=raw_text,
                seconds=int(num_samples) / SAMPLE_RATE,
            )
        )
    return candidates


def _zeroth_candidates(parquet_path: Path) -> list[Candidate]:
    table = pq.read_table(parquet_path, columns=["id", "text", "audio"]).to_pylist()
    return [
        Candidate(
            id=row["id"],
            sentence_key=row["id"],
            reference=row["text"],
            seconds=soundfile.info(io.BytesIO(row["audio"]["bytes"])).duration,
        )
        for row in table
    ]


def _download(spec: EvalSetSpec, filename: str) -> Path:
    path = hf_hub_download(
        repo_id=spec.repo_id, filename=filename, repo_type="dataset", revision=spec.revision
    )
    return Path(path)


def _list_candidates(spec: EvalSetSpec) -> list[Candidate]:
    source = _download(spec, spec.split_file)
    if spec.repo_id == "google/fleurs":
        return _fleurs_candidates(source)
    return _zeroth_candidates(source)


def _drop_hub_cache(spec: EvalSetSpec) -> None:
    cache_dir = hf_home() / "hub" / f"datasets--{spec.repo_id.replace('/', '--')}"
    if cache_dir.parent.name == "hub" and cache_dir.name.startswith("datasets--"):
        shutil.rmtree(cache_dir, ignore_errors=True)


def build_eval_ids() -> dict:
    sets = {}
    for spec in EVAL_SETS.values():
        candidates = _list_candidates(spec)
        ids = select_ids(candidates, spec.count)
        by_id = {c.id: c for c in candidates}
        sets[spec.name] = {
            "language": spec.language,
            "source": spec.repo_id,
            "revision": spec.revision,
            "split": "test",
            "candidates": len({c.sentence_key for c in candidates if c.seconds <= MAX_SECONDS}),
            "count": len(ids),
            "total_seconds": round(sum(by_id[i].seconds for i in ids), 1),
            "ids": ids,
        }
    return {
        "seed": SEED,
        "max_seconds": MAX_SECONDS,
        "selection": "후보를 seed 와 id 를 이은 글의 SHA-256 순서로 줄 세워 앞에서 count 개",
        "sets": sets,
    }


def write_eval_ids() -> dict:
    eval_ids = build_eval_ids()
    EVAL_IDS_PATH.parent.mkdir(parents=True, exist_ok=True)
    EVAL_IDS_PATH.write_text(
        json.dumps(eval_ids, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    return eval_ids


def parse_set_limits(text: str | None, set_names: Sequence[str]) -> dict[str, int | None]:
    """숫자 하나는 모든 세트에, 세트=수 목록은 세트마다, 비면 제한 없음(예: fleurs_ko=16)."""
    if not text or not text.strip():
        return dict.fromkeys(set_names)
    if "=" not in text:
        return dict.fromkeys(set_names, int(text))
    limits: dict[str, int | None] = dict.fromkeys(set_names)
    for pair in text.replace(",", " ").split():
        name, _, count = pair.partition("=")
        if name not in limits:
            message = f"모르는 세트: {name}"
            raise ValueError(message)
        limits[name] = int(count)
    return limits


def load_eval_ids() -> dict:
    return json.loads(EVAL_IDS_PATH.read_text(encoding="utf-8"))


def _wav_name(utterance_id: str) -> str:
    return f"{Path(utterance_id).stem}.wav"


def _to_mono_16k(audio: np.ndarray, sample_rate: int) -> np.ndarray:
    mono = audio.mean(axis=1) if audio.ndim > 1 else audio
    if sample_rate != SAMPLE_RATE:
        mono = librosa.resample(mono, orig_sr=sample_rate, target_sr=SAMPLE_RATE)
    return mono.astype(np.float32)


def _save_wav(path: Path, raw_bytes: bytes) -> float:
    audio, sample_rate = soundfile.read(io.BytesIO(raw_bytes), dtype="float32")
    mono = _to_mono_16k(audio, sample_rate)
    soundfile.write(path, mono, SAMPLE_RATE, subtype="PCM_16")
    return len(mono) / SAMPLE_RATE


def _fleurs_audio(tar_path: Path, wanted: set[str]) -> dict[str, bytes]:
    found = {}
    with tarfile.open(tar_path, "r:gz") as archive:
        for member in archive:
            name = Path(member.name).name
            if name in wanted and member.isfile():
                found[name] = archive.extractfile(member).read()
    return found


def _zeroth_audio(parquet_path: Path, wanted: set[str]) -> dict[str, bytes]:
    rows = pq.read_table(parquet_path, columns=["id", "audio"]).to_pylist()
    return {row["id"]: row["audio"]["bytes"] for row in rows if row["id"] in wanted}


def _references_path(set_name: str) -> Path:
    return eval_audio_dir(set_name) / "references.json"


def _extract_audio(spec: EvalSetSpec, ids: set[str]) -> dict[str, bytes]:
    if spec.audio_file is None:
        return _zeroth_audio(_download(spec, spec.split_file), ids)
    return _fleurs_audio(_download(spec, spec.audio_file), ids)


def prepare_audio(set_name: str) -> None:
    """뽑힌 녹음의 16kHz mono wav 와 정답 글을 .tmp/hf 에 둔다, 이미 있으면 받지 않는다."""
    spec = EVAL_SETS[set_name]
    ids = load_eval_ids()["sets"][set_name]["ids"]
    out_dir = eval_audio_dir(set_name)
    if _references_path(set_name).exists() and all((out_dir / _wav_name(i)).exists() for i in ids):
        return
    out_dir.mkdir(parents=True, exist_ok=True)
    candidates = {c.id: c for c in _list_candidates(spec)}
    raw = _extract_audio(spec, set(ids))
    references = {}
    for utterance_id in ids:
        seconds = _save_wav(out_dir / _wav_name(utterance_id), raw[utterance_id])
        references[utterance_id] = {
            "reference": candidates[utterance_id].reference,
            "seconds": round(seconds, 3),
        }
    _references_path(set_name).write_text(
        json.dumps(references, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    _drop_hub_cache(spec)


def load_eval_set(set_name: str, limit: int | None = None) -> list[Utterance]:
    """eval_ids.json 의 순서대로 앞에서 limit 개(없으면 전부)를 돌려준다."""
    ids = load_eval_ids()["sets"][set_name]["ids"]
    selected = ids if limit is None else ids[:limit]
    out_dir = eval_audio_dir(set_name)
    references = json.loads(_references_path(set_name).read_text(encoding="utf-8"))
    return [
        Utterance(
            id=utterance_id,
            reference=references[utterance_id]["reference"],
            audio_path=out_dir / _wav_name(utterance_id),
            seconds=references[utterance_id]["seconds"],
        )
        for utterance_id in selected
    ]


def read_audio(utterance: Utterance) -> np.ndarray:
    audio, sample_rate = soundfile.read(utterance.audio_path, dtype="float32")
    if sample_rate != SAMPLE_RATE:
        message = f"{utterance.audio_path} 의 표본 추출률이 {sample_rate} 이다"
        raise ValueError(message)
    return audio


def main() -> None:
    eval_ids = write_eval_ids()
    for name, info in eval_ids["sets"].items():
        print(f"{name}: id {info['count']}개, 후보 {info['candidates']}개", end=", ")
        print(f"총 {info['total_seconds']}초", flush=True)
        prepare_audio(name)
        saved = load_eval_set(name)
        total = sum(u.seconds for u in saved)
        print(f"  음성 {len(saved)}개, 총 {total:.1f}초를 {eval_audio_dir(name)} 에 뒀다")


if __name__ == "__main__":
    main()
