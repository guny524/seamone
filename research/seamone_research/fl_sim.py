"""라벨 없는 FL 모의실험: 기기마다 라벨 없는 발화로 학생을 학습하고 서버가 합친다.

- 학생은 whisper-small 이고 글 디코더의 LoRA 만 학습한다
- 방법: A1 진짜 정답(상한), A2 큰 모델이 단 정답
  A3 학생이 단 정답에서 믿을 만한 조각만(입력에 SpecAugment 와 소음)
- A0(학습 없음)은 돌리지 않는다
  #2 가 잰 whisper-small 의 가설 글을 같은 표본의 앞부분으로 다시 센다
- 방법마다 같은 기기 뽑기(seed)와 같은 처음 LoRA 가중치에서 시작한다
  라운드마다 뽑힌 기기가 전역 가중치에서 자기 발화를 한 바퀴 학습하고
  서버가 발화 수로 가중 평균한다(fedavg.py)
- 마지막 라운드 뒤 시험 세트의 가설 글을 .tmp/research/hyp/fl-<방법>/<세트>.jsonl 에 쌓는다
  score 와 compare 로 다시 센다
- 라운드 기록은 .tmp/research/fl_sim/<방법>.jsonl
"""

import argparse
import json
import time
from collections.abc import Sequence
from dataclasses import dataclass

import numpy as np

from seamone_research.cer import corpus_counts, normalize
from seamone_research.data import EVAL_SETS, Utterance, load_eval_set, parse_set_limits, read_audio
from seamone_research.fedavg import fedavg, sample_devices, state_megabytes
from seamone_research.fl_data import Client, load_clients
from seamone_research.fl_train import Student
from seamone_research.hypotheses import encode_row
from seamone_research.memory import release_memory
from seamone_research.paths import RUN_DIR, hypothesis_path
from seamone_research.pseudo_label import (
    RELIABLE_THRESHOLD,
    Labeled,
    load_teacher_labels,
    select_reliable,
)

STUDENT = "whisper-small"
SEED = 20261119
TRAIN_BATCH = 4
LABEL_BATCH = 8
EVAL_BATCH = 8
METHODS = ("a1", "a2", "a3")
EVAL_SETS_DEFAULT = "zeroth_ko=40 fleurs_ko=40 fleurs_en=20"


@dataclass(frozen=True)
class SimConfig:
    devices: int
    utts: int
    rounds: int
    per_round: int
    lr: float
    threshold: float
    eval_limits: dict[str, int | None]
    teacher: str = "whisper-large-v3-turbo"
    tag: str = ""


def label_by_student(student: Student, utterances: Sequence[Utterance]) -> list[Labeled]:
    """학생이 지금 전역 가중치로 정답을 달고 믿음도를 잰다."""
    labeled = []
    for start in range(0, len(utterances), LABEL_BATCH):
        chunk = utterances[start : start + LABEL_BATCH]
        waves = [read_audio(u) for u in chunk]
        texts = student.transcribe(waves, "ko")
        confidences = student.confidences(waves, texts)
        labeled += [Labeled(u.id, t, c) for u, t, c in zip(chunk, texts, confidences, strict=True)]
    return labeled


def cer_against_references(items: Sequence[Labeled], references: dict[str, str]) -> float | None:
    if not items:
        return None
    return corpus_counts((references[i.id], i.text) for i in items).cer


def confidence_and_error(items: Sequence[Labeled], references: dict[str, str]) -> list[list[float]]:
    """단 정답마다 [믿음도, 진짜 정답 대비 CER], 문턱을 어디에 둘지 살필 때 쓴다."""
    return [
        [round(i.confidence, 3), round(corpus_counts([(references[i.id], i.text)]).cer, 3)]
        for i in items
    ]


def training_pairs(
    method: str, client: Client, labels: dict[str, str]
) -> tuple[list[Utterance], list[str]]:
    """A1 은 진짜 정답, A2 와 A3 는 labels 의 정답을 쓴다(없거나 빈 글은 뺀다)."""
    if method == "a1":
        return list(client.utterances), [u.reference for u in client.utterances]
    kept = [(u, labels[u.id]) for u in client.utterances if normalize(labels.get(u.id, ""))]
    return [u for u, _ in kept], [text for _, text in kept]


def run_rounds(
    method: str,
    student: Student,
    clients: Sequence[Client],
    teacher_labels: dict[str, str],
    config: SimConfig,
    init_state: dict,
) -> list[dict]:
    """라운드를 돌고 학생을 마지막 전역 가중치로 맞춘다, 라운드마다의 기록을 돌려준다."""
    global_state = {k: v.clone() for k, v in init_state.items()}
    rng = np.random.default_rng(SEED)
    references = {u.id: u.reference for c in clients for u in c.utterances}
    records = []
    draws = sample_devices(len(clients), config.per_round, config.rounds, SEED)
    for round_number, device_indexes in enumerate(draws, start=1):
        started = time.perf_counter()
        student.load_lora_state(global_state)
        labels, record = teacher_labels, {"round": round_number, "devices": device_indexes}
        if method == "a3":
            selected = [u for i in device_indexes for u in clients[i].utterances]
            labeled = label_by_student(student, selected)
            kept = select_reliable(labeled, config.threshold)
            labels = {item.id: item.text for item in kept}
            record |= {
                "labeled": len(labeled),
                "kept": len(kept),
                "label_cer_all": cer_against_references(labeled, references),
                "label_cer_kept": cer_against_references(kept, references),
                "confidence_and_error": confidence_and_error(labeled, references),
            }
        updates, losses, train_seconds, trained = [], [], 0.0, 0
        for index in device_indexes:
            utterances, texts = training_pairs(method, clients[index], labels)
            if not utterances:
                continue
            student.load_lora_state(global_state)
            augment = rng if method == "a3" else None
            loss, seconds = student.train_client(utterances, texts, config.lr, TRAIN_BATCH, augment)
            updates.append((student.lora_state(), len(utterances)))
            losses.append(loss)
            train_seconds += seconds
            trained += len(utterances)
        if updates:
            global_state = fedavg(updates)
            record["update_mb"] = round(state_megabytes(updates[0][0]), 3)
        record |= {
            "trained_utterances": trained,
            "clients_with_update": len(updates),
            "mean_loss": sum(losses) / len(losses) if losses else None,
            "train_seconds": round(train_seconds, 2),
            "seconds_per_utterance": round(train_seconds / trained, 3) if trained else None,
            "round_seconds": round(time.perf_counter() - started, 1),
        }
        records.append(record)
        print(
            f"  {method} 라운드 {round_number}: {json.dumps(record, ensure_ascii=False)}",
            flush=True,
        )
    student.load_lora_state(global_state)
    return records


def evaluate(student: Student, name: str, eval_limits: dict[str, int | None]) -> dict[str, float]:
    """시험 세트마다 받아써 가설 글을 쌓고 CER 을 돌려준다."""
    cers = {}
    for set_name, limit in eval_limits.items():
        utterances = load_eval_set(set_name, limit)
        language = EVAL_SETS[set_name].language
        path = hypothesis_path(f"fl-{name}", set_name)
        path.parent.mkdir(parents=True, exist_ok=True)
        rows = []
        for start in range(0, len(utterances), EVAL_BATCH):
            chunk = utterances[start : start + EVAL_BATCH]
            texts = student.transcribe([read_audio(u) for u in chunk], language)
            rows += [
                encode_row(u.id, u.reference, t, u.seconds)
                for u, t in zip(chunk, texts, strict=True)
            ]
            print(f"  {name} 시험 {set_name} {len(rows)}/{len(utterances)}", flush=True)
        path.write_text("".join(rows), encoding="utf-8")
        pairs = [(u.reference, json.loads(r)["hyp"]) for u, r in zip(utterances, rows, strict=True)]
        cers[set_name] = corpus_counts(pairs).cer
    return cers


def run(methods: Sequence[str], config: SimConfig) -> None:
    clients = load_clients(config.devices, config.utts)
    teacher_labels = load_teacher_labels(config.teacher) if "a2" in methods else {}
    student = Student(STUDENT, "mps", "float16", SEED)
    init_state = student.lora_state()
    log_dir = RUN_DIR / "fl_sim"
    log_dir.mkdir(parents=True, exist_ok=True)
    for method in methods:
        started = time.perf_counter()
        records = run_rounds(method, student, clients, teacher_labels, config, init_state)
        name = f"{method}{config.tag}"
        cers = evaluate(student, name, config.eval_limits)
        summary = {
            "method": method,
            "tag": config.tag,
            "eval_cer": cers,
            "seconds": round(time.perf_counter() - started, 1),
        }
        lines = [json.dumps(r, ensure_ascii=False) for r in records] + [
            json.dumps(summary, ensure_ascii=False)
        ]
        (log_dir / f"{name}.jsonl").write_text("\n".join(lines) + "\n", encoding="utf-8")
        print(f"{name} 끝: {json.dumps(summary, ensure_ascii=False)}", flush=True)
        release_memory()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--methods", default="a2 a3")
    parser.add_argument("--devices", type=int, default=8)
    parser.add_argument("--utts", type=int, default=6)
    parser.add_argument("--rounds", type=int, default=2)
    parser.add_argument("--per-round", type=int, default=4)
    parser.add_argument("--lr", type=float, default=1e-4)
    parser.add_argument("--threshold", type=float, default=RELIABLE_THRESHOLD)
    parser.add_argument("--eval", default=EVAL_SETS_DEFAULT)
    parser.add_argument("--teacher", default="whisper-large-v3-turbo")
    parser.add_argument("--tag", default="", help="결과 이름 끝에 붙일 말(예: -lr3e-3)")
    args = parser.parse_args()
    methods = args.methods.split()
    unknown = [m for m in methods if m not in METHODS]
    if unknown:
        message = f"모르는 방법: {unknown}"
        raise SystemExit(message)
    limits = parse_set_limits(args.eval, list(EVAL_SETS))
    eval_limits = {name: n for name, n in limits.items() if n is not None}
    config = SimConfig(
        args.devices,
        args.utts,
        args.rounds,
        args.per_round,
        args.lr,
        args.threshold,
        eval_limits,
        args.teacher,
        args.tag,
    )
    run(methods, config)


if __name__ == "__main__":
    main()
