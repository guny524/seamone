import re

import pytest

from seamone_research import models, transcribe
from seamone_research.models import MODEL_SPECS


def test_every_candidate_is_pinned_to_a_commit():
    assert all(re.fullmatch(r"[0-9a-f]{40}", spec.revision) for spec in MODEL_SPECS.values())


def test_resolve_model_path_downloads_the_pinned_revision(monkeypatch):
    calls = []
    monkeypatch.setattr(
        models, "snapshot_download", lambda repo, **kw: calls.append((repo, kw)) or "/snap/x"
    )
    spec = MODEL_SPECS["whisper-small"]
    assert models.resolve_model_path(spec) == "/snap/x"
    assert calls[0][0] == "openai/whisper-small"
    assert calls[0][1]["revision"] == spec.revision


class FakeModel:
    def to(self, device):
        return self

    def eval(self):
        return self


def test_the_whisper_loader_reads_the_resolved_snapshot(monkeypatch):
    seen = {}
    monkeypatch.setattr(transcribe, "resolve_model_path", lambda spec: "/snap/abc")
    monkeypatch.setattr(
        transcribe.WhisperProcessor,
        "from_pretrained",
        staticmethod(lambda path, **kw: seen.update(processor=path)),
    )
    monkeypatch.setattr(
        transcribe.WhisperForConditionalGeneration,
        "from_pretrained",
        staticmethod(lambda path, **kw: seen.update(model=path) or FakeModel()),
    )
    transcribe.build_transcriber(MODEL_SPECS["whisper-small"], "cpu", "float32")
    assert seen == {"processor": "/snap/abc", "model": "/snap/abc"}


def test_the_qwen_loader_reads_the_resolved_snapshot(monkeypatch):
    qwen_asr = pytest.importorskip("qwen_asr")
    seen = {}
    monkeypatch.setattr(transcribe, "resolve_model_path", lambda spec: "/snap/abc")
    monkeypatch.setattr(
        qwen_asr.Qwen3ASRModel,
        "from_pretrained",
        staticmethod(lambda path, **kw: seen.update(model=path)),
    )
    transcribe.build_transcriber(MODEL_SPECS["qwen3-asr-1.7b"], "cpu", "float32")
    assert seen == {"model": "/snap/abc"}
