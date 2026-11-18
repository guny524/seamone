"""글자 오류율(CER): 글 정규화와 편집 거리, 세트 전체 합산.

정규화 규칙(정답과 가설 모두에 같게 적용한다)
1. 유니코드 NFC 로 합친다(자모로 풀어 쓴 한글을 음절로 합친다)
2. 소문자로 바꾼다
3. 문장부호(유니코드 범주 P*), 공백(Z*), 제어 문자와 서식 문자(Cc, Cf)를 지운다
4. 숫자와 기호(통화 기호 `$`, 수학 기호 `+` 같은 S*)는 그대로 둔다, 숫자를 글자로 바꾸지 않는다
   (정답이 "오" 이고 모델이 "5" 로 쓰면 오류로 센다)

세트의 CER 은 발화마다 CER 의 평균이 아니라 편집 거리의 합을 정답 글자 수의 합으로 나눈 값이다.
표준오차는 발화를 한 덩어리로 보는 비율 추정(ratio estimator)의 값이다, 표본이 작을 때 결과를 얼마나
믿을지 어림하려는 것이다.
"""

import math
import unicodedata
from collections.abc import Iterable, Sequence
from dataclasses import dataclass

import jiwer

_DROPPED_CATEGORY_PREFIXES = ("P", "Z")
_DROPPED_CATEGORIES = frozenset({"Cc", "Cf"})


def normalize(text: str) -> str:
    composed = unicodedata.normalize("NFC", text).lower()
    return "".join(char for char in composed if not _is_dropped(char))


def _is_dropped(char: str) -> bool:
    category = unicodedata.category(char)
    return category.startswith(_DROPPED_CATEGORY_PREFIXES) or category in _DROPPED_CATEGORIES


@dataclass(frozen=True)
class CerCounts:
    substitutions: int
    deletions: int
    insertions: int
    reference_chars: int

    @property
    def errors(self) -> int:
        return self.substitutions + self.deletions + self.insertions

    @property
    def cer(self) -> float:
        if self.reference_chars == 0:
            raise ValueError("정답 글자가 없어 CER 을 계산할 수 없다")
        return self.errors / self.reference_chars

    def __add__(self, other: "CerCounts") -> "CerCounts":
        return CerCounts(
            substitutions=self.substitutions + other.substitutions,
            deletions=self.deletions + other.deletions,
            insertions=self.insertions + other.insertions,
            reference_chars=self.reference_chars + other.reference_chars,
        )


EMPTY_COUNTS = CerCounts(substitutions=0, deletions=0, insertions=0, reference_chars=0)


def count_errors(reference: str, hypothesis: str) -> CerCounts:
    reference_text = normalize(reference)
    hypothesis_text = normalize(hypothesis)
    if not reference_text:
        raise ValueError("정규화한 정답이 비어 있다, 정답 글이 문장부호와 공백뿐이다")
    output = jiwer.process_characters(
        reference_text,
        hypothesis_text,
        reference_transform=jiwer.ReduceToListOfListOfChars(),
        hypothesis_transform=jiwer.ReduceToListOfListOfChars(),
    )
    return CerCounts(
        substitutions=output.substitutions,
        deletions=output.deletions,
        insertions=output.insertions,
        reference_chars=len(reference_text),
    )


def corpus_counts(pairs: Iterable[tuple[str, str]]) -> CerCounts:
    total = EMPTY_COUNTS
    for reference, hypothesis in pairs:
        total = total + count_errors(reference, hypothesis)
    return total


def ratio_estimate(errors: Sequence[float], chars: Sequence[float]) -> tuple[float, float]:
    """오류 합을 글자 합으로 나눈 값과 그 표준오차, 발화 하나를 한 덩어리로 본다.

    발화가 하나 이하면 표준오차는 nan 이다.
    """
    value = sum(errors) / sum(chars)
    count = len(errors)
    if count <= 1:
        return value, math.nan
    spread = sum((e - value * c) ** 2 for e, c in zip(errors, chars, strict=True))
    return value, math.sqrt(count / (count - 1) * spread) / sum(chars)


def corpus_standard_error(pairs: Iterable[tuple[str, str]]) -> float:
    per_utterance = [count_errors(reference, hypothesis) for reference, hypothesis in pairs]
    errors = [counts.errors for counts in per_utterance]
    chars = [counts.reference_chars for counts in per_utterance]
    return ratio_estimate(errors, chars)[1]
