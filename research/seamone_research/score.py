"""저장된 가설 글(.tmp/research/hyp)로 CER 을 계산한다.

- 모델과 세트마다: 잰 수, CER, 대체와 삭제와 삽입의 비율, 정답보다 두 배 넘게 긴 가설의 수
- 모든 모델이 같은 앞부분(eval_ids.json 순서의 앞에서부터)을 가진 만큼의 CER 을 따로 낸다
  (큰 모델의 표본을 줄였을 때도 같은 표본으로 견줄 수 있다)
"""

import argparse
import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass

from seamone_research.cer import CerCounts, corpus_counts, corpus_standard_error, normalize
from seamone_research.data import EVAL_SETS, load_eval_ids
from seamone_research.hypotheses import read_hypotheses
from seamone_research.paths import RUN_DIR, hypothesis_path

RUNAWAY_LENGTH_RATIO = 2.0


@dataclass(frozen=True)
class SetScore:
    model: str
    set_name: str
    n: int
    counts: CerCounts
    runaway: int


def runaway_count(rows: Sequence[Mapping[str, str]]) -> int:
    """정규화한 가설이 정답보다 두 배 넘게 긴 발화의 수, 길이만 견주므로 반복 검사는 아니다."""
    return sum(
        len(normalize(row["hyp"])) > RUNAWAY_LENGTH_RATIO * len(normalize(row["ref"]))
        for row in rows
    )


def score_rows(model: str, set_name: str, rows: Sequence[Mapping[str, str]]) -> SetScore:
    counts = corpus_counts((row["ref"], row["hyp"]) for row in rows)
    return SetScore(model, set_name, len(rows), counts, runaway_count(rows))


def common_prefix_length(
    ids_by_model: Mapping[str, Sequence[str]], expected_ids: Sequence[str]
) -> int:
    """모든 모델이 eval_ids.json 순서의 앞에서부터 가진 발화 수의 최솟값."""
    lengths = []
    for model, ids in ids_by_model.items():
        if list(ids) != list(expected_ids[: len(ids)]):
            message = f"{model} 의 가설 id 가 eval_ids.json 의 앞부분과 다르다"
            raise ValueError(message)
        lengths.append(len(ids))
    return min(lengths, default=0)


def shared_reference_prefix(
    rows_by_model: Mapping[str, Sequence[Mapping[str, str]]],
    expected_ids: Sequence[str],
    set_name: str,
) -> int:
    """공통 표본의 id와 CER 정규화 이후의 정답 글이 모두 같은지 확인한다."""
    ids = {model: [row["id"] for row in rows] for model, rows in rows_by_model.items()}
    common = common_prefix_length(ids, expected_ids)
    if common == 0:
        return 0

    models = iter(rows_by_model.items())
    baseline_model, baseline_rows = next(models)
    baseline_refs = [normalize(row["ref"]) for row in baseline_rows[:common]]
    for model, rows in models:
        for index, (reference, row) in enumerate(zip(baseline_refs, rows[:common], strict=True)):
            if reference != normalize(row["ref"]):
                message = (
                    f"{set_name}/{expected_ids[index]}: "
                    f"{baseline_model} 과 {model} 의 정규화한 정답이 다르다"
                )
                raise ValueError(message)
    return common


def format_score(score: SetScore) -> str:
    counts = score.counts
    total = counts.reference_chars
    return (
        f"{score.model} {score.set_name}: n={score.n} CER {counts.cer:.2%} "
        f"(대체 {counts.substitutions / total:.2%}, 삭제 {counts.deletions / total:.2%}, "
        f"삽입 {counts.insertions / total:.2%}) 정답 글자 {total}, "
        f"두 배 넘게 긴 가설 {score.runaway}"
    )


def collect(models: Sequence[str], set_names: Sequence[str]) -> list[dict]:
    expected = load_eval_ids()["sets"]
    summary = []
    for set_name in set_names:
        rows_by_model = {
            model: read_hypotheses(hypothesis_path(model, set_name)) for model in models
        }
        measured = {model: rows for model, rows in rows_by_model.items() if rows}
        if not measured:
            continue
        common = shared_reference_prefix(measured, expected[set_name]["ids"], set_name)
        for model, rows in measured.items():
            full = score_rows(model, set_name, rows)
            shared = score_rows(model, set_name, rows[:common])
            summary.append(
                {
                    "model": model,
                    "set": set_name,
                    "n": full.n,
                    "cer": full.counts.cer,
                    "substitutions": full.counts.substitutions,
                    "deletions": full.counts.deletions,
                    "insertions": full.counts.insertions,
                    "reference_chars": full.counts.reference_chars,
                    "runaway": full.runaway,
                    "cer_stderr": corpus_standard_error((r["ref"], r["hyp"]) for r in rows),
                    "common_n": common,
                    "common_cer": shared.counts.cer,
                    "common_cer_stderr": corpus_standard_error(
                        (r["ref"], r["hyp"]) for r in rows[:common]
                    ),
                }
            )
    return summary


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--models", required=True)
    parser.add_argument("--sets", default=" ".join(EVAL_SETS))
    args = parser.parse_args()
    summary = collect(args.models.split(), args.sets.split())
    for item in summary:
        print(
            f"{item['model']} {item['set']}: n={item['n']} CER {item['cer']:.2%} "
            f"(대체 {item['substitutions'] / item['reference_chars']:.2%}, "
            f"삭제 {item['deletions'] / item['reference_chars']:.2%}, "
            f"삽입 {item['insertions'] / item['reference_chars']:.2%}), "
            f"두 배 넘게 긴 가설 {item['runaway']}, 표준오차 ±{item['cer_stderr']:.2%}p, "
            f"공통 앞부분 {item['common_n']}개의 CER {item['common_cer']:.2%} "
            f"(±{item['common_cer_stderr']:.2%}p)"
        )
    RUN_DIR.mkdir(parents=True, exist_ok=True)
    (RUN_DIR / "cer_summary.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    main()
