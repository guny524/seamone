from seamone_research.pseudo_label import Labeled, mean_token_probability, select_reliable


def test_threshold_keeps_items_at_or_above():
    items = [Labeled("a", "안녕", 0.95), Labeled("b", "안녕", 0.9), Labeled("c", "안녕", 0.89)]
    assert [i.id for i in select_reliable(items, 0.9)] == ["a", "b"]


def test_empty_or_punctuation_only_text_is_dropped_even_when_sure():
    items = [Labeled("a", "", 1.0), Labeled("b", "  ", 1.0), Labeled("c", "...", 1.0)]
    assert select_reliable(items, 0.5) == []


def test_order_is_kept():
    items = [Labeled("x", "가", 0.99), Labeled("y", "나", 0.2), Labeled("z", "다", 0.91)]
    assert [i.id for i in select_reliable(items, 0.9)] == ["x", "z"]


def test_nothing_passes_a_threshold_above_every_confidence():
    assert select_reliable([Labeled("a", "가", 0.97)], 0.99) == []


def test_default_threshold_is_point_nine():
    kept = select_reliable([Labeled("a", "가", 0.9), Labeled("b", "나", 0.8999)])
    assert [i.id for i in kept] == ["a"]


def test_mean_token_probability():
    assert mean_token_probability([0.5, 1.0]) == 0.75
    assert mean_token_probability([]) == 0.0
