# Research transcriber family boundary (2026-11-27)

- Owner: dot; scope `research/seamone_research/transcribe.py`, a focused research test, and this note. Base `66a21eb07afd49f3f3b26072f5ab80be77b34c75`, target develop.
- Verify that the dispatcher rejects unsupported families before model snapshot resolution. Keep both known backends and their settings unchanged. No training, model/weights change, guard or CI policy edits, or overlap with #13/#14/#17/#18/#19.

## Evidence and enforcement
- Baseline `transcribe.py` blob `4a56b0a2925a665cbac29f07e72052bee112ba22`: a synthetic `qwen3_asr` typo called the snapshot resolver and fell through instead of raising. Two valid backend controls passed before the fix.
- `build_transcriber` now explicitly selects Whisper or Qwen before resolving any snapshot, with a model key and family in the error for unsupported values. The dispatcher owns this rule; there is no default engine and no duplicate change to the active ModelSpec revision PR.
- Focused pytest: baseline 1 failed/2 passed, fixed 3 passed. Production dispatcher imported and run; local torch/transformers import stubs, mocked resolver and constructors. No network/model weights used. Ruff check and format, compileall and diff checks pass.
- `make -C research test-offline` is blocked locally because sudo is absent; GitHub's isolated research job is the full target. Native/device/CER and actual model access were not tested. No new lint exceptions or policy changes.
