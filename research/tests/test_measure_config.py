import json

import pytest

from seamone_research import cache
from seamone_research.cer import NORMALIZATION_VERSION
from seamone_research.data import EVAL_SETS
from seamone_research.measure import (
    ensure_prefix_of_current_order,
    ensure_same_config,
    measurement_config,
)
from seamone_research.models import MODEL_SPECS

IDS = ["a.flac", "b.flac", "c.flac"]


def make_config(ids: list[str] = IDS) -> dict:
    spec = MODEL_SPECS["whisper-small"]
    return measurement_config("whisper-small", spec, "fleurs_ko", "mps", "float16", ids)


def test_config_records_model_data_and_normalization_versions():
    config = make_config()
    assert config["revision"] == MODEL_SPECS["whisper-small"].revision
    assert config["data_revision"] == EVAL_SETS["fleurs_ko"].revision
    assert config["normalization"] == NORMALIZATION_VERSION


def test_a_new_measurement_writes_its_config(tmp_path):
    path = tmp_path / "hyp" / "fleurs_ko.config.json"
    ensure_same_config(path, make_config(), has_rows=False)
    assert json.loads(path.read_text(encoding="utf-8")) == make_config()


def test_resuming_with_the_same_config_is_allowed(tmp_path):
    path = tmp_path / "x.config.json"
    ensure_same_config(path, make_config(), has_rows=False)
    ensure_same_config(path, make_config(), has_rows=True)


def test_resuming_with_another_model_revision_is_refused(tmp_path):
    path = tmp_path / "x.config.json"
    ensure_same_config(path, make_config(), has_rows=False)
    changed = make_config() | {"revision": "0" * 40}
    with pytest.raises(ValueError, match="revision"):
        ensure_same_config(path, changed, has_rows=True)


def test_resuming_with_another_data_revision_or_normalization_is_refused(tmp_path):
    path = tmp_path / "x.config.json"
    ensure_same_config(path, make_config(), has_rows=False)
    for key in ("data_revision", "normalization", "dtype"):
        with pytest.raises(ValueError, match=key):
            ensure_same_config(path, make_config() | {key: "다름"}, has_rows=True)


def test_hypotheses_without_a_config_cannot_be_resumed(tmp_path):
    with pytest.raises(ValueError, match="설정 파일"):
        ensure_same_config(tmp_path / "x.config.json", make_config(), has_rows=True)


def test_a_truncated_config_without_any_hypothesis_is_made_again(tmp_path):
    path = tmp_path / "x.config.json"
    path.write_text(json.dumps(make_config())[:25], encoding="utf-8")
    ensure_same_config(path, make_config(), has_rows=False)
    assert json.loads(path.read_text(encoding="utf-8")) == make_config()
    assert sorted(p.name for p in tmp_path.iterdir()) == ["x.config.json"]


def test_a_truncated_config_with_hypotheses_stops_and_names_the_file(tmp_path):
    path = tmp_path / "x.config.json"
    path.write_text(json.dumps(make_config())[:25], encoding="utf-8")
    with pytest.raises(ValueError, match=r"x\.config\.json"):
        ensure_same_config(path, make_config(), has_rows=True)


def test_a_config_that_is_not_an_object_is_treated_as_broken(tmp_path):
    path = tmp_path / "x.config.json"
    path.write_text("[1, 2]", encoding="utf-8")
    ensure_same_config(path, make_config(), has_rows=False)
    with pytest.raises(ValueError, match="객체가 아니다"):
        path.write_text("[1, 2]", encoding="utf-8")
        ensure_same_config(path, make_config(), has_rows=True)


def test_the_config_is_written_through_a_temporary_name(tmp_path, monkeypatch):
    path = tmp_path / "x.config.json"
    seen = []
    original = cache.publish_partial

    def spy(self, target):
        seen.append(self.name)
        return original(self, target)

    monkeypatch.setattr(cache, "publish_partial", spy)
    ensure_same_config(path, make_config(), has_rows=False)
    assert seen == ["x.config.json.part"]


def test_the_config_holds_a_hash_of_the_whole_id_list_whatever_the_limit(tmp_path):
    assert make_config()["ids_count"] == 3
    assert make_config(IDS)["ids_sha256"] == make_config(list(IDS))["ids_sha256"]
    assert make_config(IDS)["ids_sha256"] != make_config(IDS[::-1])["ids_sha256"]
    assert make_config(IDS)["ids_sha256"] != make_config(IDS[:2])["ids_sha256"]


def test_resuming_with_a_reordered_or_changed_sample_list_is_refused(tmp_path):
    path = tmp_path / "x.config.json"
    ensure_same_config(path, make_config(), has_rows=False)
    with pytest.raises(ValueError, match="ids_sha256"):
        ensure_same_config(path, make_config(["b.flac", "a.flac", "c.flac"]), has_rows=True)


def test_existing_hypotheses_must_be_the_front_of_the_current_order(tmp_path):
    path = tmp_path / "x.jsonl"
    ensure_prefix_of_current_order(path, [], IDS)
    ensure_prefix_of_current_order(path, ["a.flac", "b.flac"], IDS)
    ensure_prefix_of_current_order(path, ["a.flac"], [*IDS, "d.flac"])
    with pytest.raises(ValueError, match="앞부분과 다르다"):
        ensure_prefix_of_current_order(path, ["a.flac"], ["b.flac", "a.flac"])
    with pytest.raises(ValueError, match="앞부분과 다르다"):
        ensure_prefix_of_current_order(path, [*IDS, "x.flac"], IDS)
