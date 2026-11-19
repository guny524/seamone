import pytest

from seamone_research.data import Candidate, parse_set_limits, select_ids


def candidates(count: int, seconds: float = 10.0) -> list[Candidate]:
    return [
        Candidate(
            id=f"utt{i:03d}.wav", sentence_key=f"s{i}", reference=f"문장 {i}", seconds=seconds
        )
        for i in range(count)
    ]


def test_selection_is_deterministic_for_the_same_seed():
    pool = candidates(50)

    assert select_ids(pool, 10, seed=1) == select_ids(pool, 10, seed=1)


def test_selection_changes_with_the_seed():
    pool = candidates(50)

    assert select_ids(pool, 10, seed=1) != select_ids(pool, 10, seed=2)


def test_selection_does_not_depend_on_candidate_order():
    pool = candidates(50)

    assert select_ids(pool, 10, seed=1) == select_ids(list(reversed(pool)), 10, seed=1)


def test_smaller_selection_is_the_front_part_of_the_larger_one():
    pool = candidates(50)

    assert select_ids(pool, 5, seed=1) == select_ids(pool, 20, seed=1)[:5]


def test_recordings_longer_than_the_limit_are_never_selected():
    pool = [*candidates(10), Candidate("long.wav", "long", "긴 문장", seconds=30.5)]

    assert "long.wav" not in select_ids(pool, 10, seed=1, max_seconds=30.0)


def test_a_sentence_read_by_several_speakers_is_selected_once_with_the_first_file_name():
    pool = [
        Candidate(f"{speaker}{i}.wav", f"s{i}", f"문장 {i}", 5.0)
        for i in range(10)
        for speaker in ("b", "a", "c")
    ]

    assert sorted(select_ids(pool, 10, seed=1)) == [f"a{i}.wav" for i in range(10)]


def test_references_that_are_empty_after_normalization_are_never_selected():
    pool = [*candidates(5), Candidate("blank.wav", "blank", "... !", 5.0)]

    assert "blank.wav" not in select_ids(pool, 5, seed=1)


def test_too_few_candidates_is_an_error():
    with pytest.raises(ValueError, match="뽑을 수 없다"):
        select_ids(candidates(3), 5, seed=1)


def test_a_single_number_applies_to_every_set():
    assert parse_set_limits("16", ["a", "b"]) == {"a": 16, "b": 16}


def test_per_set_limits_leave_unlisted_sets_unlimited():
    assert parse_set_limits("a=8", ["a", "b"]) == {"a": 8, "b": None}


def test_several_per_set_limits_can_be_separated_by_spaces_or_commas():
    assert parse_set_limits("a=8 b=4,c=2", ["a", "b", "c"]) == {"a": 8, "b": 4, "c": 2}


@pytest.mark.parametrize("text", ["", "  ", None])
def test_empty_limit_means_no_limit(text):
    assert parse_set_limits(text, ["a"]) == {"a": None}


def test_a_limit_for_an_unknown_set_is_an_error():
    with pytest.raises(ValueError, match="모르는 세트"):
        parse_set_limits("x=3", ["a"])
