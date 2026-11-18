"""같은 발화에서 모델을 견준다: 여러 세트를 합친 CER 과 두 모델의 CER 차이.

- 세트마다 모든 모델이 eval_ids.json 순서의 앞에서부터 같이 가진 발화만 쓴다
- 차이는 같은 발화에서 모델 a 의 오류 수에서 모델 b 의 오류 수를 뺀 값을
  정답 글자 수의 합으로 나눈 값이다(음수면 a 가 오류가 적다)
- 표준오차는 발화 쌍을 한 덩어리로 본 비율 추정이다
"""

import argparse
from collections.abc import Sequence

from seamone_research.cer import CerCounts, count_errors, ratio_estimate
from seamone_research.data import EVAL_SETS, load_eval_ids
from seamone_research.hypotheses import read_hypotheses
from seamone_research.paths import hypothesis_path
from seamone_research.score import common_prefix_length


def pooled_rate(per_utterance: Sequence[CerCounts]) -> tuple[float, float]:
    errors = [counts.errors for counts in per_utterance]
    chars = [counts.reference_chars for counts in per_utterance]
    return ratio_estimate(errors, chars)


def paired_difference(
    counts_a: Sequence[CerCounts], counts_b: Sequence[CerCounts]
) -> tuple[float, float]:
    differences = [a.errors - b.errors for a, b in zip(counts_a, counts_b, strict=True)]
    chars = [a.reference_chars for a in counts_a]
    return ratio_estimate(differences, chars)


def counts_on_common_prefix(
    models: Sequence[str], set_names: Sequence[str]
) -> dict[str, list[CerCounts]]:
    """모델마다 세트들의 공통 앞부분 발화의 오류 수를 세트 순서대로 이어 붙인다."""
    expected = load_eval_ids()["sets"]
    result: dict[str, list[CerCounts]] = {model: [] for model in models}
    for set_name in set_names:
        rows = {model: read_hypotheses(hypothesis_path(model, set_name)) for model in models}
        ids = {model: [row["id"] for row in rows[model]] for model in models}
        common = common_prefix_length(ids, expected[set_name]["ids"])
        for model in models:
            result[model] += [count_errors(r["ref"], r["hyp"]) for r in rows[model][:common]]
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--models", required=True)
    parser.add_argument("--sets", default=" ".join(EVAL_SETS))
    parser.add_argument("--pair", default=None, help="차이를 낼 두 모델 'a b'(a 에서 b 를 뺀다)")
    args = parser.parse_args()
    models, set_names = args.models.split(), args.sets.split()
    counts = counts_on_common_prefix(models, set_names)
    for model in models:
        rate, stderr = pooled_rate(counts[model])
        count = len(counts[model])
        print(f"{model}: 세트 {set_names} 합산 CER {rate:.2%} (±{stderr:.2%}p), 발화 {count}개")
    if args.pair:
        a, b = args.pair.split()
        pair_counts = counts_on_common_prefix([a, b], set_names)
        delta, stderr = paired_difference(pair_counts[a], pair_counts[b])
        count = len(pair_counts[a])
        print(f"{a} 에서 {b} 를 뺀 CER 차이: {delta * 100:+.2f}%p (±{stderr:.2%}p)", end=", ")
        print(f"같은 발화 {count}개")


if __name__ == "__main__":
    main()
