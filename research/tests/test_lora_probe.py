import re

import pytest
import torch

from seamone_research.lora_probe import judge_probe, lora_targets

WHISPER_NAMES = {
    "model.encoder.layers.0.self_attn.q_proj": False,
    "model.encoder.layers.11.self_attn.v_proj": False,
    "model.decoder.layers.0.self_attn.q_proj": True,
    "model.decoder.layers.3.encoder_attn.v_proj": True,
    "model.decoder.layers.3.encoder_attn.k_proj": False,
}
QWEN_NAMES = {
    "audio_tower.layers.0.self_attn.q_proj": False,
    "model.layers.0.self_attn.q_proj": True,
    "model.layers.27.self_attn.v_proj": True,
    "model.layers.27.self_attn.k_proj": False,
}


@pytest.mark.parametrize(
    ("family", "names"), [("whisper", WHISPER_NAMES), ("qwen3-asr", QWEN_NAMES)]
)
def test_decoder_scope_matches_only_text_decoder_attention_modules(family, names):
    pattern = lora_targets(family, "decoder")

    for name, expected in names.items():
        assert bool(re.fullmatch(pattern, name)) is expected, name


def test_all_scope_names_every_q_and_v_projection():
    assert lora_targets("whisper", "all") == ["q_proj", "v_proj"]


def test_unknown_scope_is_an_error():
    with pytest.raises(ValueError, match="모르는 범위"):
        lora_targets("whisper", "encoder")


def test_judge_probe_passes_when_every_parameter_has_a_finite_gradient_and_losses_are_finite():
    grads = [torch.ones(2), torch.zeros(3)]
    judgement = judge_probe(grads, [0.5, 0.4])
    assert judgement.ok
    assert judgement.params_without_grad == 0


def test_judge_probe_fails_when_a_trainable_parameter_has_no_gradient():
    judgement = judge_probe([torch.ones(2), None, torch.ones(1)], [0.5, 0.4])
    assert not judgement.ok
    assert judgement.params_without_grad == 1
    assert judgement.grads_finite


def test_judge_probe_fails_on_a_non_finite_loss_in_any_step():
    assert not judge_probe([torch.ones(2)], [0.5, float("nan")]).ok
    assert not judge_probe([torch.ones(2)], [float("inf"), 0.5]).ok


def test_judge_probe_fails_on_a_non_finite_gradient():
    judgement = judge_probe([torch.ones(2), torch.tensor([1.0, float("inf")])], [0.5])
    assert not judgement.ok
    assert not judgement.grads_finite


def test_judge_probe_fails_without_any_gradient_or_loss():
    assert not judge_probe([], [0.5]).ok
    assert not judge_probe([torch.ones(2)], []).ok
