import hashlib
import json
from types import SimpleNamespace

import pytest
import torch

from seamone_research.verified_step import load_fixture, optimizer_step


def fixture(tmp_path):
    audio = tmp_path / "sample.wav"
    audio.write_bytes(b"fixture")
    data = {
        "kind": "public_human_labelled_fixture",
        "source": "test-corpus",
        "revision": "fixed",
        "split": "train",
        "utterance_id": "training.wav",
        "text": "사람이 단 정답",
        "audio": "sample.wav",
        "sha256": hashlib.sha256(audio.read_bytes()).hexdigest(),
    }
    path = tmp_path / "fixture.json"
    path.write_text(json.dumps(data))
    return path, data


def test_training_fixture_rejects_evaluation_and_modified_source(tmp_path):
    path, data = fixture(tmp_path)
    evaluation = {"sets": {"test": {"ids": ["evaluation.wav"]}}}
    assert load_fixture(path, evaluation)[0] == data
    evaluation["sets"]["test"]["ids"].append("training.wav")
    with pytest.raises(ValueError, match="평가"):
        load_fixture(path, evaluation)
    evaluation["sets"]["test"]["ids"] = []
    (tmp_path / "sample.wav").write_bytes(b"changed")
    with pytest.raises(ValueError, match="SHA-256"):
        load_fixture(path, evaluation)


def test_research_input_cannot_be_an_app_record_or_held_out_split(tmp_path):
    path, data = fixture(tmp_path)
    data["kind"] = "app_record"
    path.write_text(json.dumps(data))
    with pytest.raises(ValueError, match="공개 train"):
        load_fixture(path, {"sets": {}})
    data["kind"] = "public_human_labelled_fixture"
    data["split"] = "test"
    path.write_text(json.dumps(data))
    with pytest.raises(ValueError, match="공개 train"):
        load_fixture(path, {"sets": {}})


class TinyAdapter(torch.nn.Module):
    def __init__(self):
        super().__init__()
        self.base = torch.nn.Parameter(torch.tensor([2.0]), requires_grad=False)
        self.lora_weight = torch.nn.Parameter(torch.tensor([0.1]))

    def forward(self, value):
        return SimpleNamespace(loss=((self.base + self.lora_weight) * value).square().sum())


def test_real_optimizer_changes_adapter_and_preserves_frozen_base():
    model = TinyAdapter()
    proof = optimizer_step(model, {"value": torch.tensor([1.0])})
    assert proof["optimizer_steps"] == 1
    assert proof["adapter_delta_l2"] > 0
    assert proof["base_sha256_before"] == proof["base_sha256_after"]
    assert torch.equal(model.base, torch.tensor([2.0]))
    assert not torch.equal(model.lora_weight, torch.tensor([0.1]))
