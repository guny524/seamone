import json

import pytest

from seamone_research.cer import NORMALIZATION_VERSION
from seamone_research.data import EVAL_SETS
from seamone_research.measure import ensure_same_config, measurement_config
from seamone_research.models import MODEL_SPECS


def make_config() -> dict:
    spec = MODEL_SPECS["whisper-small"]
    return measurement_config("whisper-small", spec, "fleurs_ko", "mps", "float16")


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
