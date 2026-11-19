import pytest
import torch

from seamone_research.fedavg import fedavg, sample_devices, state_megabytes


def make_state(value: float) -> dict[str, torch.Tensor]:
    return {"a": torch.full((2, 2), value), "b": torch.tensor([value, value + 1.0])}


def test_average_of_identical_states_is_unchanged():
    merged = fedavg([(make_state(3.0), 5), (make_state(3.0), 7)])
    assert torch.allclose(merged["a"], make_state(3.0)["a"])
    assert torch.allclose(merged["b"], make_state(3.0)["b"])


def test_weights_follow_utterance_counts():
    merged = fedavg([(make_state(0.0), 1), (make_state(3.0), 2)])
    assert torch.allclose(merged["a"], torch.full((2, 2), 2.0))
    assert torch.allclose(merged["b"], torch.tensor([2.0, 3.0]))


def test_equal_counts_give_the_plain_mean():
    merged = fedavg([(make_state(1.0), 4), (make_state(5.0), 4)])
    assert torch.allclose(merged["a"], torch.full((2, 2), 3.0))


def test_single_client_returns_its_own_state():
    merged = fedavg([(make_state(7.0), 3)])
    assert torch.allclose(merged["b"], make_state(7.0)["b"])


def test_client_with_zero_utterances_has_no_say():
    merged = fedavg([(make_state(10.0), 0), (make_state(2.0), 6)])
    assert torch.allclose(merged["a"], torch.full((2, 2), 2.0))


def test_result_is_float32_even_for_half_inputs():
    half = {"a": torch.ones(2, dtype=torch.float16)}
    assert fedavg([(half, 1)])["a"].dtype == torch.float32


def test_no_updates_is_an_error():
    with pytest.raises(ValueError, match="없다"):
        fedavg([])


def test_zero_total_utterances_is_an_error():
    with pytest.raises(ValueError, match="0 이하"):
        fedavg([(make_state(1.0), 0)])


def test_mismatched_names_are_an_error():
    with pytest.raises(ValueError, match="이름"):
        fedavg([(make_state(1.0), 1), ({"a": torch.zeros(2, 2)}, 1)])


def test_state_megabytes_counts_bytes_of_every_tensor():
    state = {
        "a": torch.zeros(250_000, dtype=torch.float32),
        "b": torch.zeros(500_000, dtype=torch.float16),
    }
    assert state_megabytes(state) == pytest.approx(2.0)


def test_sample_devices_is_deterministic_and_sized():
    first = sample_devices(8, 4, 3, seed=1)
    assert first == sample_devices(8, 4, 3, seed=1)
    assert len(first) == 3
    for devices in first:
        assert len(devices) == 4
        assert len(set(devices)) == 4
        assert all(0 <= d < 8 for d in devices)


def test_sample_devices_differs_by_seed():
    assert sample_devices(20, 5, 4, seed=1) != sample_devices(20, 5, 4, seed=2)


def test_sample_devices_cannot_take_more_than_exist():
    with pytest.raises(ValueError, match="뽑을 수 없다"):
        sample_devices(3, 4, 1, seed=1)
