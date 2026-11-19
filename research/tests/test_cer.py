import pytest

from seamone_research.cer import corpus_counts, corpus_standard_error, count_errors, normalize


def test_identical_text_has_zero_cer():
    counts = count_errors("안녕하세요 여러분", "안녕하세요 여러분")

    assert counts.errors == 0
    assert counts.cer == 0.0


def test_text_differing_only_in_spaces_and_punctuation_has_zero_cer():
    counts = count_errors("안녕하세요, 여러분!", "안녕 하세요 여러분")

    assert counts.cer == 0.0


def test_text_differing_only_in_case_has_zero_cer():
    assert count_errors("Hello World", "hello world").cer == 0.0


def test_one_changed_hangul_syllable_is_one_substitution_in_five_characters():
    counts = count_errors("안녕하세요", "안녕하새요")

    assert counts.substitutions == 1
    assert counts.deletions == 0
    assert counts.insertions == 0
    assert counts.reference_chars == 5
    assert counts.cer == pytest.approx(0.2)


def test_empty_hypothesis_is_all_deletions():
    counts = count_errors("안녕하세요", "")

    assert counts.deletions == 5
    assert counts.cer == 1.0


def test_extra_characters_are_insertions_and_can_push_cer_above_one():
    counts = count_errors("가", "가나다")

    assert counts.insertions == 2
    assert counts.cer == 2.0


def test_hangul_and_english_mixed_text_counts_each_character():
    # 정답 "나는ai를좋아해"(8글자), 가설 "나는에이아이를좋아해"(10글자), 바뀐 곳은 "ai" -> "에이아이"
    counts = count_errors("나는 AI를 좋아해", "나는 에이아이를 좋아해")

    assert counts.reference_chars == 8
    assert counts.errors == 4
    assert counts.cer == pytest.approx(0.5)


def test_digits_are_not_converted_to_words():
    # 정답이 "오" 이고 모델이 "5" 로 쓰면 오류다
    assert count_errors("오", "5").cer == 1.0


def test_symbols_other_than_punctuation_are_kept():
    # "$" 는 문장부호가 아니라 통화 기호라 남는다
    counts = count_errors("$5", "5")

    assert counts.reference_chars == 2
    assert counts.deletions == 1


def test_empty_reference_after_normalization_is_rejected():
    with pytest.raises(ValueError, match="정답"):
        count_errors("...", "아무 말")


def test_corpus_cer_divides_summed_errors_by_summed_reference_chars():
    pairs = [("가나다라마", "가나다라마"), ("가나", "가")]

    counts = corpus_counts(pairs)

    # 발화마다 CER 의 평균이면 (0 + 0.5) / 2 = 0.25 지만 세트 전체는 1 / 7 이다
    assert counts.errors == 1
    assert counts.reference_chars == 7
    assert counts.cer == pytest.approx(1 / 7)


def test_corpus_counts_of_no_pairs_is_empty():
    assert corpus_counts([]).reference_chars == 0


@pytest.mark.parametrize(
    ("raw", "expected"),
    [
        ("Hello, World!", "helloworld"),
        ("“안녕”… 하세요.", "안녕하세요"),
        ("a\tb\nc​d", "abcd"),
        ("가격은 $5 입니다", "가격은$5입니다"),
        ("1,000원", "1000원"),
    ],
)
def test_normalize_removes_punctuation_and_whitespace_and_lowercases(raw, expected):
    assert normalize(raw) == expected


def test_normalize_composes_decomposed_hangul():
    decomposed = "한"  # 한 을 자모로 풀어 쓴 꼴

    assert normalize(decomposed) == "한"


def test_standard_error_is_zero_when_every_utterance_has_the_same_error_rate():
    pairs = [("가나다라", "가나다"), ("마바사아", "마바사")]

    assert corpus_standard_error(pairs) == pytest.approx(0.0)


def test_standard_error_grows_when_errors_pile_up_in_one_utterance():
    even = [("가나다라", "가나다"), ("마바사아", "마바사")]
    uneven = [("가나다라", "가나다라"), ("마바사아", "마바")]

    assert corpus_standard_error(uneven) > corpus_standard_error(even)


def test_standard_error_of_two_utterances_matches_the_hand_calculation():
    # 정답 글자 4와 4, 오류 0과 2 -> CER 0.25, 잔차 (0 - 1)^2 + (2 - 1)^2 = 2, n/(n-1) = 2
    # 표준오차 = sqrt(2 * 2) / 8 = 0.25
    pairs = [("가나다라", "가나다라"), ("마바사아", "마바")]

    assert corpus_standard_error(pairs) == pytest.approx(0.25)


def test_standard_error_of_a_single_utterance_is_not_a_number():
    assert corpus_standard_error([("가나", "가")]) != corpus_standard_error([("가나", "가")])
