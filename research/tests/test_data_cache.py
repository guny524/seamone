import dataclasses
import io
import json

import numpy as np
import pytest
import soundfile

from seamone_research import data
from seamone_research.data import EVAL_SETS

IDS = ["a.flac", "b.flac"]


@pytest.fixture
def hf_home(tmp_path, monkeypatch):
    monkeypatch.setenv("HF_HOME", str(tmp_path))
    return tmp_path


def make_cache(set_name: str, ids: list[str]) -> None:
    out_dir = data.eval_audio_dir(set_name)
    out_dir.mkdir(parents=True)
    references = {}
    for utterance_id in ids:
        (out_dir / data._wav_name(utterance_id)).write_bytes(b"wav")
        references[utterance_id] = {"reference": "글", "seconds": 1.0}
    (out_dir / "references.json").write_text(json.dumps(references), encoding="utf-8")
    manifest = data.manifest_for(EVAL_SETS[set_name], ids)
    (out_dir / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")


def valid(set_name: str = "zeroth_ko", ids: list[str] = IDS) -> bool:
    return data.cache_is_valid(set_name, EVAL_SETS[set_name], ids)


def test_a_finished_cache_of_the_same_version_is_reused(hf_home):
    make_cache("zeroth_ko", IDS)
    assert valid()


def test_a_manifest_from_another_data_revision_is_rejected(hf_home):
    make_cache("zeroth_ko", IDS)
    other = dataclasses.replace(EVAL_SETS["zeroth_ko"], revision="0" * 40)
    assert not data.cache_is_valid("zeroth_ko", other, IDS)


def test_a_different_id_list_is_rejected(hf_home):
    make_cache("zeroth_ko", IDS)
    assert not valid(ids=[*IDS, "c.flac"])
    assert not valid(ids=IDS[::-1])


def test_a_truncated_reference_file_is_rejected(hf_home):
    make_cache("zeroth_ko", IDS)
    path = data.eval_audio_dir("zeroth_ko") / "references.json"
    path.write_text(path.read_text(encoding="utf-8")[:30], encoding="utf-8")
    assert not valid()


def test_a_missing_wav_or_reference_is_rejected(hf_home):
    make_cache("zeroth_ko", IDS)
    out_dir = data.eval_audio_dir("zeroth_ko")
    (out_dir / data._wav_name("b.flac")).unlink()
    assert not valid()
    make_cache("fleurs_en", IDS)
    path = data.eval_audio_dir("fleurs_en") / "references.json"
    path.write_text(json.dumps({"a.flac": {"reference": "글", "seconds": 1.0}}), encoding="utf-8")
    assert not valid("fleurs_en")


def test_a_cache_without_a_manifest_is_rejected(hf_home):
    make_cache("zeroth_ko", IDS)
    (data.eval_audio_dir("zeroth_ko") / "manifest.json").unlink()
    assert not valid()


def test_dropping_the_raw_cache_leaves_the_shared_hub_alone(hf_home):
    shared = hf_home / "hub" / "datasets--kresnik--zeroth_korean" / "blobs"
    shared.mkdir(parents=True)
    (shared / "train-shard").write_bytes(b"keep")
    raw = data.raw_cache_dir("zeroth_ko") / "datasets--kresnik--zeroth_korean" / "blobs"
    raw.mkdir(parents=True)
    (raw / "test-shard").write_bytes(b"drop")
    data.drop_raw_cache("zeroth_ko")
    assert (shared / "train-shard").read_bytes() == b"keep"
    assert not data.raw_cache_dir("zeroth_ko").exists()


def test_downloads_go_to_the_dedicated_cache_not_the_shared_hub(hf_home, monkeypatch):
    calls = []
    monkeypatch.setattr(data, "hf_hub_download", lambda *args, **kw: calls.append(kw) or "/x")
    data._download(EVAL_SETS["zeroth_ko"], "data/test.parquet")
    assert calls[0]["cache_dir"] == data.raw_cache_dir("zeroth_ko")
    assert hf_home / "hub" not in calls[0]["cache_dir"].parents


def test_remove_inside_refuses_the_root_and_outside_paths_and_links_out(tmp_path):
    root, outside = tmp_path / "root", tmp_path / "outside"
    (root / "inner").mkdir(parents=True)
    outside.mkdir()
    (root / "link").symlink_to(outside)
    data.remove_inside(root, root / "inner")
    assert not (root / "inner").exists()
    for target in (root, outside, root / "link"):
        with pytest.raises(ValueError, match="안이 아니라서"):
            data.remove_inside(root, target)
    assert outside.exists()


def test_save_wav_leaves_only_the_finished_file(tmp_path):
    buffer = io.BytesIO()
    soundfile.write(buffer, np.zeros(16_000, dtype=np.float32), 16_000, format="WAV")
    seconds = data._save_wav(tmp_path / "a.wav", buffer.getvalue())
    assert seconds == pytest.approx(1.0)
    assert sorted(p.name for p in tmp_path.iterdir()) == ["a.wav"]
    assert soundfile.info(tmp_path / "a.wav").frames == 16_000
