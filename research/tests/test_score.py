import pytest

from seamone_research.score import common_prefix_length, runaway_count, score_rows


def rows(*pairs: tuple[str, str]) -> list[dict[str, str]]:
    return [{"ref": ref, "hyp": hyp} for ref, hyp in pairs]


def test_set_score_is_total_edits_over_total_reference_chars():
    score = score_rows("m", "s", rows(("가나다라마", "가나다라마"), ("가나", "가")))

    assert score.n == 2
    assert score.counts.cer == pytest.approx(1 / 7)


def test_runaway_counts_hypotheses_more_than_twice_as_long_as_the_reference():
    pairs = rows(("가나", "가나"), ("가나", "가나가나가나"), ("가나", "가나가나"))

    assert runaway_count(pairs) == 1


def test_common_prefix_is_the_shortest_measured_length_among_models():
    expected = ["a", "b", "c", "d"]

    assert common_prefix_length({"small": ["a", "b", "c", "d"], "large": ["a", "b"]}, expected) == 2


def test_common_prefix_is_an_error_when_ids_are_not_the_front_of_the_expected_order():
    with pytest.raises(ValueError, match="앞부분과 다르다"):
        common_prefix_length({"small": ["a", "c"]}, ["a", "b", "c"])


def test_common_prefix_of_no_models_is_zero():
    assert common_prefix_length({}, ["a"]) == 0
