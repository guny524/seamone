"""가설 글 파일(.tmp/research/hyp/<모델>/<세트>.jsonl)을 읽고 쓴다.

한 줄은 {"id", "ref", "hyp", "seconds"} 이고 eval_ids.json 의 순서대로 앞에서부터 쌓인다.
개행 없이 잘린 마지막 줄만 버린다(repair_hypotheses 가 마지막 완성 줄까지 자른다).
개행으로 끝난 행이 깨졌으면 마지막 행이라도 줄 번호를 적은 오류로 멈춘다.
"""

import json
from pathlib import Path


def encode_row(utterance_id: str, reference: str, hypothesis: str, seconds: float) -> str:
    row = {"id": utterance_id, "ref": reference, "hyp": hypothesis, "seconds": seconds}
    return json.dumps(row, ensure_ascii=False) + "\n"


def _parse(path: Path) -> tuple[list[dict], bool]:
    """(해석된 행들, 파일을 고쳐 써야 하는지), 끝 줄이 깨졌거나 끝 개행이 없으면 고쳐 써야 한다."""
    if not path.exists():
        return [], False
    data = path.read_bytes()
    lines = data.split(b"\n")
    ends_with_newline = data.endswith(b"\n") or not data
    if data.endswith(b"\n"):
        lines.pop()
    rows = []
    for number, raw in enumerate(lines, start=1):
        if not raw.strip():
            continue
        try:
            rows.append(json.loads(raw.decode("utf-8")))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            if number == len(lines) and not ends_with_newline:
                return rows, True
            message = f"{path}:{number} 줄이 깨졌다({type(error).__name__}), 파일을 고쳐야 한다"
            raise ValueError(message) from error
    return rows, not ends_with_newline


def read_hypotheses(path: Path) -> list[dict]:
    """읽기만 한다. 개행 없이 잘린 마지막 줄만 제외한다."""
    return _parse(path)[0]


def repair_hypotheses(path: Path) -> list[dict]:
    """이어 쓰기 전에 잘린 끝 줄만 버리고 끝 개행을 맞춘다."""
    rows, needs_repair = _parse(path)
    if needs_repair:
        text = "".join(json.dumps(row, ensure_ascii=False) + "\n" for row in rows)
        partial = path.with_name(path.name + ".part")
        partial.write_text(text, encoding="utf-8")
        partial.replace(path)
    return rows
