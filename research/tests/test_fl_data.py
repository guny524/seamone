import pytest

from seamone_research.fl_data import FlCandidate, select_clients


def make_candidates() -> list[FlCandidate]:
    candidates = []
    for speaker in range(1, 6):
        candidates += [
            FlCandidate(f"{speaker}-{k}", speaker, f"문장 {speaker} {k}", 5.0) for k in range(10)
        ]
    candidates += [FlCandidate(f"6-{k}", 6, "문장", 5.0) for k in range(2)]
    candidates += [FlCandidate(f"7-{k}", 7, "문장", 5.0) for k in range(10)]
    return candidates


def test_picks_requested_number_of_speakers_and_utterances():
    chosen = select_clients(make_candidates(), 3, 4)
    assert len(chosen) == 3
    for speaker, ids in chosen.items():
        assert len(ids) == 4
        assert all(i.startswith(f"{speaker}-") for i in ids)


def test_speakers_with_too_few_utterances_are_not_picked():
    chosen = select_clients(make_candidates(), 6, 5)
    assert 6 not in chosen


def test_excluded_speakers_are_not_picked():
    chosen = select_clients(make_candidates(), 3, 5, excluded_speakers={1, 2, 3})
    assert set(chosen) == {4, 5, 7}


def test_selection_is_deterministic_and_depends_on_seed():
    first = select_clients(make_candidates(), 3, 4, seed=1)
    assert first == select_clients(make_candidates(), 3, 4, seed=1)
    assert first != select_clients(make_candidates(), 3, 4, seed=2)


def test_too_long_and_empty_utterances_are_skipped():
    candidates = [FlCandidate(f"1-{k}", 1, "문장", 31.0 if k < 2 else 5.0) for k in range(5)]
    candidates += [FlCandidate("1-9", 1, "...", 5.0)]
    chosen = select_clients(candidates, 1, 3)
    assert sorted(chosen[1]) == ["1-2", "1-3", "1-4"]


def test_not_enough_speakers_is_an_error():
    with pytest.raises(ValueError, match="뽑을 수 없다"):
        select_clients(make_candidates(), 9, 5)
