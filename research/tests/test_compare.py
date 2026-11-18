import math

import pytest

from seamone_research.cer import CerCounts, ratio_estimate
from seamone_research.compare import paired_difference, pooled_rate


def counts(errors: int, chars: int) -> CerCounts:
    return CerCounts(substitutions=errors, deletions=0, insertions=0, reference_chars=chars)


def test_ratio_estimate_of_one_utterance_has_no_standard_error():
    value, stderr = ratio_estimate([1], [4])

    assert value == pytest.approx(0.25)
    assert math.isnan(stderr)


def test_pooled_rate_matches_the_hand_calculation():
    # 오류 0과 2, 정답 글자 4와 4 -> 0.25, 표준오차 sqrt(2 * 2) / 8 = 0.25
    rate, stderr = pooled_rate([counts(0, 4), counts(2, 4)])

    assert rate == pytest.approx(0.25)
    assert stderr == pytest.approx(0.25)


def test_paired_difference_is_negative_when_model_a_has_fewer_errors():
    delta, _ = paired_difference([counts(0, 4), counts(1, 4)], [counts(1, 4), counts(2, 4)])

    assert delta == pytest.approx(-0.25)


def test_paired_difference_standard_error_matches_the_hand_calculation():
    # 같은 발화에서 오류 차이가 -1 과 +1, 정답 글자 4와 4 -> 차이 0, 표준오차 sqrt(2 * 2) / 8 = 0.25
    delta, stderr = paired_difference([counts(0, 4), counts(2, 4)], [counts(1, 4), counts(1, 4)])

    assert delta == pytest.approx(0.0)
    assert stderr == pytest.approx(0.25)


def test_paired_difference_of_identical_results_is_zero_with_no_spread():
    same = [counts(1, 4), counts(2, 5), counts(0, 3)]

    delta, stderr = paired_difference(same, same)

    assert delta == 0.0
    assert stderr == 0.0
