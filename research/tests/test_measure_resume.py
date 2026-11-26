import json
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from threading import Event

import numpy as np
import pytest

from seamone_research import measure, paths
from seamone_research.data import Utterance
from seamone_research.hypotheses import encode_row, read_hypotheses
from seamone_research.models import MODEL_SPECS

SET = "fleurs_ko"
OTHER_SET = "fleurs_en"
MODEL = "whisper-small"
OTHER_MODEL = "whisper-base"


class FakeTranscriber:
    def transcribe(self, audio, language):
        return ["글"] * len(audio)

    def close(self):
        pass


def use_samples(monkeypatch, tmp_path, ids):
    def load_eval_set(set_name, limit=None):
        chosen = ids if limit is None else ids[:limit]
        return [Utterance(i, "정답", tmp_path / f"{i}.wav", 1.0) for i in chosen]

    monkeypatch.setattr(measure, "load_eval_set", load_eval_set)
    monkeypatch.setattr(
        measure,
        "load_eval_ids",
        lambda: {"sets": {name: {"ids": ids} for name in (SET, OTHER_SET)}},
    )
    monkeypatch.setattr(paths, "RUN_DIR", tmp_path)
    monkeypatch.setattr(measure, "read_audio", lambda utterance: np.zeros(160, dtype=np.float32))
    monkeypatch.setattr(measure, "device_allocated_mib", lambda device: 0.0)
    monkeypatch.setattr(measure, "peak_process_mib", lambda: 0.0)


def run(limit, model=MODEL, set_name=SET, transcriber=None):
    spec = MODEL_SPECS[model]
    return measure.measure_set(
        model, set_name, transcriber or FakeTranscriber(), spec, limit, "mps", "float16"
    )


def hypothesis_ids(tmp_path):
    return [row["id"] for row in read_hypotheses(tmp_path / "hyp" / MODEL / f"{SET}.jsonl")]


def test_resuming_with_a_larger_n_is_allowed_and_keeps_the_sample_order(monkeypatch, tmp_path):
    ids = ["a", "b", "c", "d"]
    use_samples(monkeypatch, tmp_path, ids)
    run(2)
    run(4)
    assert hypothesis_ids(tmp_path) == ids


def test_the_same_sample_resumes_and_has_nothing_more_to_do(monkeypatch, tmp_path):
    use_samples(monkeypatch, tmp_path, ["a", "b"])
    assert run(2) is not None
    assert run(2) is None


def test_a_reordered_sample_list_refuses_to_resume_and_writes_nothing(monkeypatch, tmp_path):
    use_samples(monkeypatch, tmp_path, ["a", "b"])
    run(1)
    use_samples(monkeypatch, tmp_path, ["b", "a"])
    with pytest.raises(ValueError, match="ids_sha256"):
        run(2)
    assert hypothesis_ids(tmp_path) == ["a"]


def test_hypotheses_outside_the_current_front_refuse_to_resume_even_with_a_matching_config(
    monkeypatch, tmp_path
):
    use_samples(monkeypatch, tmp_path, ["a", "b", "c"])
    run(1)
    path = tmp_path / "hyp" / MODEL / f"{SET}.jsonl"
    path.write_text(encode_row("z", "정답", "글", 1.0), encoding="utf-8")
    with pytest.raises(ValueError, match="앞부분과 다르다"):
        run(3)


def test_a_truncated_config_without_hypotheses_does_not_block_a_restart(monkeypatch, tmp_path):
    use_samples(monkeypatch, tmp_path, ["a", "b"])
    config_path = tmp_path / "hyp" / MODEL / f"{SET}.config.json"
    config_path.parent.mkdir(parents=True)
    config_path.write_text('{"model": "whisper-sm', encoding="utf-8")
    assert run(2) is not None
    assert json.loads(config_path.read_text(encoding="utf-8"))["set"] == SET
    assert hypothesis_ids(tmp_path) == ["a", "b"]


@pytest.mark.parametrize("other_set", [SET, OTHER_SET])
def test_one_writer_per_model_without_blocking_other_models(monkeypatch, tmp_path, other_set):
    use_samples(monkeypatch, tmp_path, ["a"])
    entered = Event()
    release = Event()

    class PausedTranscriber(FakeTranscriber):
        def transcribe(self, audio, language):
            entered.set()
            assert release.wait(10)
            return super().transcribe(audio, language)

    with ThreadPoolExecutor(max_workers=1) as executor:
        first = executor.submit(run, 1, MODEL, SET, PausedTranscriber())
        try:
            assert entered.wait(10)
            with pytest.raises(RuntimeError, match="측정 중"):
                run(1, MODEL, other_set)
            assert run(1, OTHER_MODEL, other_set) is not None
        finally:
            release.set()
            assert first.result(timeout=10) is not None

    assert hypothesis_ids(tmp_path) == ["a"]
    assert run(1) is None


def test_timing_publish_preserves_existing_file_on_failed_replace(monkeypatch, tmp_path):
    use_samples(monkeypatch, tmp_path, ["a"])
    monkeypatch.setattr(measure, "prepare_audio", lambda _: None)
    monkeypatch.setattr(measure, "build_transcriber", lambda *_: FakeTranscriber())
    path = paths.hypothesis_dir(MODEL) / "timing.json"
    path.parent.mkdir(parents=True)
    previous = b'[{"set":"previous"}]\n'
    path.write_bytes(previous)
    original_replace = Path.replace

    def replace_timing(source, destination):
        if destination == path:
            with pytest.raises(RuntimeError, match="측정 중"):
                run(1, MODEL, OTHER_SET)
            message = "simulated timing publish failure"
            raise OSError(message)
        return original_replace(source, destination)

    with monkeypatch.context() as patch:
        patch.setattr(Path, "replace", replace_timing)
        with pytest.raises(OSError, match="timing publish failure"):
            measure.measure_model(MODEL, [SET], {SET: 1}, None, None)

    assert path.read_bytes() == previous
    assert hypothesis_ids(tmp_path) == ["a"]
    measure.measure_model(MODEL, [SET, OTHER_SET], {SET: 1, OTHER_SET: 1}, None, None)
    assert [row["set"] for row in json.loads(path.read_bytes())] == ["previous", OTHER_SET]
