"""시험 세트: 고정한 방법으로 뽑은 표본의 id 목록과 16kHz mono 음성.

- 후보: 세트의 test split 에서 30초 이하(Whisper 의 입력 창)이고 정규화한 정답이 비지 않은 녹음
- 같은 문장을 여러 사람이 읽은 녹음(FLEURS)은 file_name 이 가장 앞선 하나만 후보로 둔다
- 뽑기: seed 와 id 를 이은 글의 SHA-256 순서로 줄 세워 앞에서 count 개를 뽑는다
  이 순서가 eval_ids.json 의 순서다
  앞에서부터 잘라 쓰면 모든 모델이 같은 표본의 앞부분을 쓴다
- 음성은 .tmp/hf 에만 둔다, test split 한 파일만 받아 뽑힌 녹음만 꺼내고 원본은 지운다
- 원본은 세트마다 전용 cache(.tmp/hf/eval-raw/<세트>)에 받고 그 폴더만 지운다
  공유 hub/ 는 건드리지 않는다
- 준비한 음성 폴더에는 manifest.json(데이터 revision, id 목록의 해시, wav 크기)을 마지막에 쓴다
  재사용 전에 대조한다
- 같은 세트의 준비는 세트별 잠금 파일(.tmp/hf/eval-raw/<세트>.lock)을 쥐고 하나씩만 한다
"""

import contextlib
import fcntl
import hashlib
import io
import json
import tarfile
from collections.abc import Iterator, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

import librosa
import numpy as np
import pyarrow.parquet as pq
import soundfile
from huggingface_hub import hf_hub_download

from seamone_research import cache
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


def raw_cache_dir(set_name: str) -> Path:
    """준비 단계가 받는 원본 조각의 전용 cache, 다른 실행이 읽는 공유 hub/ 와 섞이지 않는다."""
    return hf_home() / "eval-raw" / set_name


@contextlib.contextmanager
def set_lock(set_name: str) -> Iterator[None]:
    """같은 세트를 준비하는 실행끼리 하나씩 하게 한다, 잠금은 프로세스가 끝나면 풀린다."""
    lock_path = hf_home() / "eval-raw" / f"{set_name}.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    with lock_path.open("w") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        try:
            yield
        finally:
            fcntl.flock(handle, fcntl.LOCK_UN)


def drop_raw_cache(set_name: str) -> None:
    target = raw_cache_dir(set_name)
    if target.exists():
        cache.remove_inside(hf_home() / "eval-raw", target)


def _download(spec: EvalSetSpec, filename: str) -> Path:
    path = hf_hub_download(
        repo_id=spec.repo_id,
        filename=filename,
        repo_type="dataset",
        revision=spec.revision,
        cache_dir=raw_cache_dir(spec.name),
    )
    return Path(path)


def _list_candidates(spec: EvalSetSpec) -> list[Candidate]:
    source = _download(spec, spec.split_file)
    if spec.repo_id == "google/fleurs":
        return _fleurs_candidates(source)
    return _zeroth_candidates(source)


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
    """임시 이름으로 쓴 뒤 이름을 바꿔 완성된 파일만 공개한다."""
    audio, sample_rate = soundfile.read(io.BytesIO(raw_bytes), dtype="float32")
    mono = _to_mono_16k(audio, sample_rate)
    partial = path.with_name(path.name + ".part")
    soundfile.write(partial, mono, SAMPLE_RATE, subtype="PCM_16", format="WAV")
    partial.replace(path)
    return len(mono) / SAMPLE_RATE


def _write_json_atomically(path: Path, value: object) -> None:
    partial = path.with_name(path.name + ".part")
    partial.write_text(json.dumps(value, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    partial.replace(path)


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


def wav_sizes_of(set_name: str, ids: Sequence[str]) -> dict[str, int]:
    """wav 파일 이름마다의 크기, 없는 파일이 있으면 FileNotFoundError 다."""
    out_dir = eval_audio_dir(set_name)
    return {_wav_name(i): (out_dir / _wav_name(i)).stat().st_size for i in ids}


def manifest_for(spec: EvalSetSpec, ids: Sequence[str], wav_sizes: Mapping[str, int]) -> dict:
    digest = hashlib.sha256("\n".join(ids).encode("utf-8")).hexdigest()
    return {
        "repo": spec.repo_id,
        "revision": spec.revision,
        "count": len(ids),
        "ids_sha256": digest,
        "wav_sizes": dict(wav_sizes),
    }


def _has_reference(entry: object) -> bool:
    return (
        isinstance(entry, dict)
        and isinstance(entry.get("reference"), str)
        and isinstance(entry.get("seconds"), int | float)
    )


def cache_is_valid(set_name: str, spec: EvalSetSpec, ids: Sequence[str]) -> bool:
    """manifest 가 같고 정답과 wav(크기도 같다)가 필요한 id 마다 있을 때만 참이다."""
    out_dir = eval_audio_dir(set_name)
    try:
        manifest = json.loads((out_dir / "manifest.json").read_text(encoding="utf-8"))
        references = json.loads(_references_path(set_name).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return False
    saved_sizes = manifest.get("wav_sizes") if isinstance(manifest, dict) else None
    if not isinstance(saved_sizes, dict) or not isinstance(references, dict):
        return False
    if manifest != manifest_for(spec, ids, saved_sizes):
        return False
    try:
        actual = wav_sizes_of(set_name, ids)
    except OSError:
        return False
    return actual == saved_sizes and all(_has_reference(references.get(i)) for i in ids)


def prepare_audio(set_name: str) -> None:
    """뽑힌 녹음의 wav 와 정답 글을 .tmp/hf 에 둔다, 같은 판의 완성본이 있으면 받지 않는다."""
    spec = EVAL_SETS[set_name]
    ids = load_eval_ids()["sets"][set_name]["ids"]
    if cache_is_valid(set_name, spec, ids):
        return
    with set_lock(set_name):
        if not cache_is_valid(set_name, spec, ids):  # 기다리는 동안 다른 실행이 끝냈을 수 있다
            _build_audio(set_name, spec, ids)


def _build_audio(set_name: str, spec: EvalSetSpec, ids: Sequence[str]) -> None:
    out_dir = eval_audio_dir(set_name)
    out_dir.mkdir(parents=True, exist_ok=True)
    cache.remove_inside(out_dir, out_dir / "manifest.json", file=True, missing_ok=True)
    candidates = {c.id: c for c in _list_candidates(spec)}
    raw = _extract_audio(spec, set(ids))
    references = {}
    for utterance_id in ids:
        seconds = _save_wav(out_dir / _wav_name(utterance_id), raw[utterance_id])
        references[utterance_id] = {
            "reference": candidates[utterance_id].reference,
            "seconds": round(seconds, 3),
        }
    _write_json_atomically(_references_path(set_name), references)
    sizes = wav_sizes_of(set_name, ids)
    _write_json_atomically(out_dir / "manifest.json", manifest_for(spec, ids, sizes))
    drop_raw_cache(set_name)


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
