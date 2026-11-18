"""모델 가중치를 미리 받는다, 측정 중인 모델의 GPU 시간을 내려받기에 쓰지 않으려는 것이다.

safetensors 와 설정 파일만 받는다(.bin 사본과 fp32 사본은 받지 않는다).
"""

import argparse

from huggingface_hub import snapshot_download

from seamone_research.models import MODEL_SPECS

ALLOW_PATTERNS = ["*.json", "*.txt", "model.safetensors", "model-*.safetensors"]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--models", required=True)
    args = parser.parse_args()
    for model in args.models.split():
        path = snapshot_download(MODEL_SPECS[model].repo_id, allow_patterns=ALLOW_PATTERNS)
        print(f"{model}: {path}", flush=True)


if __name__ == "__main__":
    main()
