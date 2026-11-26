import pytest

from seamone_research.hypotheses import encode_row, read_hypotheses, repair_hypotheses


def line(index: int) -> str:
    return encode_row(f"id{index}", f"정답 {index}", f"가설 {index}", 1.5)


def ids(rows: list[dict]) -> list[str]:
    return [row["id"] for row in rows]


def test_normal_file_is_read_and_left_alone(tmp_path):
    path = tmp_path / "a.jsonl"
    path.write_text(line(1) + line(2), encoding="utf-8")
    assert ids(read_hypotheses(path)) == ["id1", "id2"]
    assert ids(repair_hypotheses(path)) == ["id1", "id2"]
    assert path.read_text(encoding="utf-8") == line(1) + line(2)


def test_truncated_last_line_is_dropped_and_the_file_is_cut_so_appending_works(tmp_path):
    path = tmp_path / "a.jsonl"
    path.write_text(line(1) + line(2)[:20], encoding="utf-8")
    assert ids(read_hypotheses(path)) == ["id1"]
    assert ids(repair_hypotheses(path)) == ["id1"]
    assert path.read_text(encoding="utf-8") == line(1)
    with path.open("a", encoding="utf-8") as out:
        out.write(line(3))
    assert ids(read_hypotheses(path)) == ["id1", "id3"]


def test_last_line_cut_inside_a_multibyte_character_is_dropped(tmp_path):
    path = tmp_path / "a.jsonl"
    second = line(2).encode("utf-8")
    cut = second.index("가".encode()) + 1
    path.write_bytes(line(1).encode("utf-8") + second[:cut])
    assert ids(repair_hypotheses(path)) == ["id1"]
    assert path.read_text(encoding="utf-8") == line(1)


def test_corrupt_middle_line_stops_with_its_line_number(tmp_path):
    path = tmp_path / "a.jsonl"
    path.write_text(line(1) + "{깨짐\n" + line(3), encoding="utf-8")
    with pytest.raises(ValueError, match=":2 줄이 깨졌다"):
        read_hypotheses(path)
    with pytest.raises(ValueError, match=":2 줄이 깨졌다"):
        repair_hypotheses(path)


def test_empty_and_missing_files_have_no_rows(tmp_path):
    empty = tmp_path / "empty.jsonl"
    empty.write_text("", encoding="utf-8")
    assert read_hypotheses(empty) == []
    assert repair_hypotheses(empty) == []
    assert read_hypotheses(tmp_path / "missing.jsonl") == []


def test_complete_last_line_without_a_newline_is_kept_and_a_newline_is_added(tmp_path):
    path = tmp_path / "a.jsonl"
    path.write_text(line(1) + line(2).rstrip("\n"), encoding="utf-8")
    assert ids(read_hypotheses(path)) == ["id1", "id2"]
    assert ids(repair_hypotheses(path)) == ["id1", "id2"]
    assert path.read_text(encoding="utf-8") == line(1) + line(2)


def test_blank_lines_are_ignored(tmp_path):
    path = tmp_path / "a.jsonl"
    path.write_text(line(1) + "\n" + line(2), encoding="utf-8")
    assert ids(read_hypotheses(path)) == ["id1", "id2"]


@pytest.mark.parametrize("reader", [read_hypotheses, repair_hypotheses])
@pytest.mark.parametrize("damage", [b"{broken\n", b'{"id":"\xff"}\n'])
def test_newline_terminated_corrupt_tail_is_rejected_without_rewriting(tmp_path, reader, damage):
    path = tmp_path / "a.jsonl"
    original = line(1).encode("utf-8") + damage
    path.write_bytes(original)

    with pytest.raises(ValueError, match=r"a\.jsonl:2 줄이 깨졌다"):
        reader(path)

    assert path.read_bytes() == original
