"""기기 발화에 다는 정답: A2 는 큰 모델이 한 번 달고, A3 는 학생이 달고 믿을 만한 것만 쓴다.

- A2: 이미 받아 둔 큰 모델(기본 whisper-large-v3-turbo)이 모든 기기 발화에 달고 내린다
  결과는 .tmp/research/pseudo/a2-<모델>.json 이고 이어서 쓴다
- A3: 학생이 단 정답의 평균 토큰 확률이 문턱(기본 0.9) 이상인 것만 학습에 쓴다(select_reliable)
"""

import argparse
import json
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from seamone_research.cer import corpus_counts, normalize
from seamone_research.data import read_audio
from seamone_research.fl_data import load_clients
from seamone_research.models import MODEL_SPECS
from seamone_research.paths import RUN_DIR
from seamone_research.transcribe import build_transcriber

RELIABLE_THRESHOLD = 0.9
PSEUDO_DIR = RUN_DIR / "pseudo"


@dataclass(frozen=True)
class Labeled:
    id: str
    text: str
    confidence: float


def mean_token_probability(probabilities: Sequence[float]) -> float:
    """정답 조각의 평균 토큰 확률, 토큰이 없으면 0 이다."""
    return sum(probabilities) / len(probabilities) if probabilities else 0.0


def select_reliable(
    items: Sequence[Labeled], threshold: float = RELIABLE_THRESHOLD
) -> list[Labeled]:
    """글이 비지 않고 믿음도가 문턱 이상인 조각만 남긴다(문턱과 같으면 남긴다)."""
    return [item for item in items if normalize(item.text) and item.confidence >= threshold]


def teacher_labels_path(model_key: str) -> Path:
    return PSEUDO_DIR / f"a2-{model_key}.json"


def load_teacher_labels(model_key: str) -> dict[str, str]:
    path = teacher_labels_path(model_key)
    return json.loads(path.read_text(encoding="utf-8")) if path.exists() else {}


def label_with_teacher(model_key: str) -> dict[str, str]:
    spec = MODEL_SPECS[model_key]
    utterances = [u for client in load_clients() for u in client.utterances]
    labels = load_teacher_labels(model_key)
    todo = [u for u in utterances if u.id not in labels]
    if not todo:
        return labels
    path = teacher_labels_path(model_key)
    path.parent.mkdir(parents=True, exist_ok=True)
    transcriber = build_transcriber(spec, spec.device, spec.dtype)
    try:
        for start in range(0, len(todo), spec.batch_size):
            batch = todo[start : start + spec.batch_size]
            texts = transcriber.transcribe([read_audio(u) for u in batch], "ko")
            labels.update({u.id: text for u, text in zip(batch, texts, strict=True)})
            path.write_text(
                json.dumps(labels, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
            )
            print(f"  정답 달기 {len(labels)}/{len(utterances)}", flush=True)
    finally:
        transcriber.close()
    return labels


def label_quality(utterances_by_id: dict[str, str], labels: dict[str, str]) -> float:
    """기기 발화의 진짜 정답 대비 단 정답의 CER."""
    pairs = [(utterances_by_id[i], text) for i, text in labels.items() if i in utterances_by_id]
    return corpus_counts(pairs).cer


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", default="whisper-large-v3-turbo", choices=sorted(MODEL_SPECS))
    args = parser.parse_args()
    labels = label_with_teacher(args.model)
    references = {u.id: u.reference for c in load_clients() for u in c.utterances}
    cer = label_quality(references, labels)
    print(f"{args.model} 가 단 정답 {len(labels)}개, 진짜 정답 대비 CER {cer:.2%}")


if __name__ == "__main__":
    main()
