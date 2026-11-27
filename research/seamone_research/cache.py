"""전용 cache의 삭제 입구, 공유 HF_HOME과 hub는 지우지 않는다."""

import shutil
from pathlib import Path

from seamone_research.paths import hf_home


def remove_inside(
    root: Path, target: Path, *, file: bool = False, missing_ok: bool = False
) -> None:
    """root의 자식만 지운다, file이면 unlink하고 디렉터리는 기존처럼 오류를 무시한다."""
    resolved_root, resolved_target = root.resolve(), target.resolve()
    shared = hf_home().resolve()
    hub = (shared / "hub").resolve()
    if (
        resolved_target in (shared, hub)
        or resolved_target in shared.parents
        or hub in resolved_target.parents
    ):
        message = f"{target} 은 공유 HF_HOME 또는 hub라서 지우지 않는다"
        raise ValueError(message)
    owned = (shared / "eval-raw", shared / "seamone_eval")
    if not any(resolved_root == area or area in resolved_root.parents for area in owned):
        message = f"{root} 은 전용 eval-raw 또는 seamone_eval cache가 아니라서 지우지 않는다"
        raise ValueError(message)
    if resolved_target == resolved_root or resolved_root not in resolved_target.parents:
        message = f"{target} 은 {root} 안이 아니라서 지우지 않는다"
        raise ValueError(message)
    if file:
        target.unlink(missing_ok=missing_ok)
    else:
        shutil.rmtree(resolved_target, ignore_errors=True)


def publish_partial(partial: Path, target: Path) -> None:
    """같은 폴더의 target.part 파일만 완성된 이름으로 바꾼다."""
    if (
        partial != target.with_name(target.name + ".part")
        or partial.parent.resolve() != target.parent.resolve()
    ):
        message = "원자적 쓰기는 같은 폴더의 .part 파일만 공개한다"
        raise ValueError(message)
    shared = hf_home().resolve()
    hub = (shared / "hub").resolve()
    if any(
        path in (shared, hub) or hub in path.parents
        for path in (partial.resolve(), target.resolve())
    ):
        message = "공유 HF_HOME 또는 hub 파일은 바꾸지 않는다"
        raise ValueError(message)
    if partial.is_symlink() or target.is_symlink() or not partial.is_file():
        message = "원자적 쓰기의 원본은 symlink가 아닌 일반 파일이어야 한다"
        raise ValueError(message)
    partial.replace(target)
