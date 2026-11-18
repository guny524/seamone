"""research/ 가 읽고 쓰는 자리.

- 가중치와 시험 음성: HF_HOME(메인 레포의 .tmp/hf), research/Makefile 이 정해 준다
- 가설 글과 측정 시간: 이 레포의 .tmp/research, 작업 컴퓨터를 옮길 때 버려도 되는 것이다
- eval_ids.json: research/data, git 에 둔다
"""

import os
from pathlib import Path

RESEARCH_DIR = Path(__file__).resolve().parents[1]
REPO_DIR = RESEARCH_DIR.parent
EVAL_IDS_PATH = RESEARCH_DIR / "data" / "eval_ids.json"
RUN_DIR = REPO_DIR / ".tmp" / "research"


def hf_home() -> Path:
    value = os.environ.get("HF_HOME")
    if not value:
        message = "HF_HOME 이 비어 있다, make -C research <target> 으로 돌려 .tmp/hf 를 쓰게 한다"
        raise RuntimeError(message)
    return Path(value)


def eval_audio_dir(set_name: str) -> Path:
    return hf_home() / "seamone_eval" / set_name


def hypothesis_dir(model: str) -> Path:
    return RUN_DIR / "hyp" / model


def hypothesis_path(model: str, set_name: str) -> Path:
    return hypothesis_dir(model) / f"{set_name}.jsonl"
