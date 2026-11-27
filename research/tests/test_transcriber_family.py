from dataclasses import replace
from unittest.mock import Mock

import pytest
import torch

from seamone_research import transcribe
from seamone_research.models import MODEL_SPECS


@pytest.fixture
def transcriber_calls(monkeypatch):
    resolve = Mock(return_value="/synthetic/snapshot")
    whisper = Mock()
    qwen = Mock()
    monkeypatch.setattr(transcribe, "resolve_model_path", resolve)
    monkeypatch.setattr(transcribe, "WhisperTranscriber", whisper)
    monkeypatch.setattr(transcribe, "QwenAsrTranscriber", qwen)
    return resolve, whisper, qwen


def test_unknown_family_is_rejected_before_resolving_or_loading(transcriber_calls):
    resolve, whisper, qwen = transcriber_calls
    spec = replace(MODEL_SPECS["qwen3-asr-0.6b"], family="qwen3_asr")

    try:
        with pytest.raises(ValueError, match="qwen3_asr"):
            transcribe.build_transcriber(spec, "cpu", "float32")
    finally:
        resolve.assert_not_called()
        whisper.assert_not_called()
        qwen.assert_not_called()


@pytest.mark.parametrize("key", ["whisper-base", "qwen3-asr-0.6b"])
def test_known_family_preserves_constructor_inputs(key, transcriber_calls):
    resolve, whisper, qwen = transcriber_calls
    spec = MODEL_SPECS[key]

    built = transcribe.build_transcriber(spec, "cpu", "float32")

    resolve.assert_called_once_with(spec)
    args = ("/synthetic/snapshot", "cpu", torch.float32)
    if key == "whisper-base":
        assert built is whisper.return_value
        whisper.assert_called_once_with(*args)
        qwen.assert_not_called()
    else:
        assert built is qwen.return_value
        qwen.assert_called_once_with(*args, spec.batch_size)
        whisper.assert_not_called()
