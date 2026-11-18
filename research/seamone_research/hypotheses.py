"""가설 글 파일(.tmp/research/hyp/<모델>/<세트>.jsonl)을 읽고 쓴다.

한 줄은 {"id", "ref", "hyp", "seconds"} 이고 eval_ids.json 의 순서대로 앞에서부터 쌓인다.
"""

import json
from pathlib import Path


def read_hypotheses(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def encode_row(utterance_id: str, reference: str, hypothesis: str, seconds: float) -> str:
    row = {"id": utterance_id, "ref": reference, "hyp": hypothesis, "seconds": seconds}
    return json.dumps(row, ensure_ascii=False) + "\n"
