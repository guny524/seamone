"""Download one short human-labelled FLEURS Korean train item, never an evaluation item.

The archive is streamed and closed after one matching member. Neither the complete archive
nor any speech/text is tracked. Preparing this fixture does not verify anything in the app.
"""

import argparse
import hashlib
import io
import json
import tarfile
from pathlib import Path
from urllib.request import urlopen

import soundfile

from seamone_research.data import FLEURS_REVISION, SAMPLE_RATE
from seamone_research.paths import EVAL_IDS_PATH
from seamone_research.verified_step import load_fixture

MAX_TSV_BYTES = 4 * 1024 * 1024
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024
MAX_FIXTURE_SECONDS = 12
BASE_URL = f"https://huggingface.co/datasets/google/fleurs/resolve/{FLEURS_REVISION}/data/ko_kr"


def prepare(output: Path) -> Path:
    output.mkdir(parents=True, exist_ok=True)
    manifest = output / "fixture.json"
    evaluation = json.loads(EVAL_IDS_PATH.read_text(encoding="utf-8"))
    if manifest.exists():
        load_fixture(manifest, evaluation)
        return manifest
    excluded = {i for spec in evaluation["sets"].values() for i in spec["ids"]}
    with urlopen(f"{BASE_URL}/train.tsv", timeout=60) as response:
        raw_tsv = response.read(MAX_TSV_BYTES + 1)
    if len(raw_tsv) > MAX_TSV_BYTES:
        message = "train.tsv가 허용 크기를 넘었습니다"
        raise ValueError(message)
    candidates = {}
    for line in raw_tsv.decode().splitlines():
        _, name, text, _, _, count, _ = line.split("\t")
        eligible_length = 0 < int(count) <= SAMPLE_RATE * MAX_FIXTURE_SECONDS
        if name not in excluded and text.strip() and eligible_length:
            candidates[name] = text
    with (
        urlopen(f"{BASE_URL}/audio/train.tar.gz", timeout=60) as response,
        tarfile.open(fileobj=response, mode="r|gz") as archive,
    ):
        for member in archive:
            if archive.offset > MAX_ARCHIVE_BYTES:
                break
            name = Path(member.name).name
            if not member.isfile() or name not in candidates or member.size > MAX_TSV_BYTES:
                continue
            handle = archive.extractfile(member)
            if handle is None:
                continue
            wave, rate = soundfile.read(io.BytesIO(handle.read()), dtype="float32")
            if rate != SAMPLE_RATE or wave.ndim != 1:
                continue
            audio = output / "fixture.wav"
            soundfile.write(audio, wave, rate, subtype="PCM_16")
            data = {
                "kind": "public_human_labelled_fixture",
                "source": "google/fleurs:ko_kr",
                "revision": FLEURS_REVISION,
                "split": "train",
                "utterance_id": name,
                "text": candidates[name],
                "audio": audio.name,
                "sha256": hashlib.sha256(audio.read_bytes()).hexdigest(),
                "source_tsv_sha256": hashlib.sha256(raw_tsv).hexdigest(),
            }
            manifest.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
            load_fixture(manifest, evaluation)
            return manifest
    message = "stream의 앞 64MB에서 fixture를 찾지 못했습니다"
    raise RuntimeError(message)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    print(prepare(args.output))


if __name__ == "__main__":
    main()
