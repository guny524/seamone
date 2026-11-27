import math

import pytest

import seamone_research.compare as comparison
import seamone_research.score as scoring
from seamone_research.cer import CerCounts, ratio_estimate
from seamone_research.compare import paired_difference, pooled_rate


def counts(errors: int, chars: int) -> CerCounts:
    return CerCounts(substitutions=errors, deletions=0, insertions=0, reference_chars=chars)


def test_ratio_estimate_of_one_utterance_has_no_standard_error():
    value, stderr = ratio_estimate([1], [4])

    assert value == pytest.approx(0.25)
    assert math.isnan(stderr)


def test_pooled_rate_matches_the_hand_calculation():
    rate, stderr = pooled_rate([counts(0, 4), counts(2, 4)])

    assert rate == pytest.approx(0.25)
    assert stderr == pytest.approx(0.25)


def test_paired_difference_is_negative_when_model_a_has_fewer_errors():
    delta, _ = paired_difference([counts(0, 4), counts(1, 4)], [counts(1, 4), counts(2, 4)])

    assert delta == pytest.approx(-0.25)


def test_paired_difference_standard_error_matches_the_hand_calculation():
    delta, stderr = paired_difference([counts(0, 4), counts(2, 4)], [counts(1, 4), counts(1, 4)])

    assert delta == pytest.approx(0.0)
    assert stderr == pytest.approx(0.25)


def test_paired_difference_of_identical_results_is_zero_with_no_spread():
    same = [counts(1, 4), counts(2, 5), counts(0, 3)]

    delta, stderr = paired_difference(same, same)

    assert delta == 0.0
    assert stderr == 0.0


@pytest.mark.parametrize("compare", ["paired", "score"])
def test_shared_cer_rejects_mismatched_references_with_the_same_id(monkeypatch, compare):
    module = comparison if compare == "paired" else scoring
    monkeypatch.setattr(module, "load_eval_ids", lambda: {"sets": {"s": {"ids": ["u"]}}})
    monkeypatch.setattr(module, "hypothesis_path", lambda model, _set: model)
    records = {
        "a": [{"id": "u", "ref": "가나", "hyp": "가나"}],
        "b": [{"id": "u", "ref": "다라", "hyp": "가나"}],
    }
    monkeypatch.setattr(module, "read_hypotheses", records.__getitem__)
    run = comparison.counts_on_common_prefix if compare == "paired" else scoring.collect

    with pytest.raises(ValueError, match=r"s/u: .*a.*b"):
        run(["a", "b"], ["s"])


def test_shared_cer_accepts_references_equivalent_after_normalization(monkeypatch):
    monkeypatch.setattr(comparison, "load_eval_ids", lambda: {"sets": {"s": {"ids": ["u", "v"]}}})
    monkeypatch.setattr(comparison, "hypothesis_path", lambda model, _set: model)
    records = {
        "a": [{"id": "u", "ref": "가 나!", "hyp": "가나"}, {"id": "v", "ref": "다", "hyp": "다"}],
        "b": [{"id": "u", "ref": "가나", "hyp": "가나"}],
    }
    monkeypatch.setattr(comparison, "read_hypotheses", records.__getitem__)

    counts_by_model = comparison.counts_on_common_prefix(["a", "b"], ["s"])

    assert counts_by_model == {"a": [counts(0, 2)], "b": [counts(0, 2)]}
