import re

import pytest

from seamone_research.lora_probe import lora_targets

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
