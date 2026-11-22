import numpy as np
import torch

from seamone_research.fl_train import add_noise, spec_augment


def test_add_noise_keeps_length_and_adds_noise_at_the_asked_level():
    rng = np.random.default_rng(0)
    wave = np.sin(np.linspace(0, 400, 16_000)).astype(np.float32)
    noisy = add_noise(wave, rng, snr_db=20.0)
    assert noisy.shape == wave.shape
    noise_power = float(np.mean((noisy - wave) ** 2))
    assert noise_power == np.float32(noise_power)
    ratio = float(np.mean(wave**2)) / noise_power
    assert 50 < ratio < 200


def test_spec_augment_masks_without_touching_the_input():
    rng = np.random.default_rng(0)
    features = torch.randn(2, 80, 300)
    before = features.clone()
    out = spec_augment(features, rng, [300, 200])
    assert out.shape == features.shape
    assert torch.equal(features, before)
    assert (out != features).any()


def test_spec_augment_time_masks_stay_inside_the_audio():
    rng = np.random.default_rng(1)
    features = torch.randn(1, 80, 3000)
    changed = spec_augment(features, rng, [100]) != features
    after_audio = changed[0, :, 100:]
    # 100 프레임 뒤에서는 주파수 띠만 바뀌므로 열마다 바뀐 멜 칸이 같다
    assert bool((after_audio == after_audio[:, :1]).all())
