"""Copy a project tree into a build directory with shutil.copytree."""

from __future__ import annotations

import shutil
from pathlib import Path

DEFAULT_IGNORES = ("__pycache__", "*.pyc", ".git", "*.tmp")


class CopyError(ValueError):
    """Raised when the source is missing or the destination is inside it."""


def copy_project(
    src: Path, dst: Path, extra_ignores: tuple[str, ...] = ()
) -> list[str]:
    """Copy src into dst, skipping ignored names, and list the copied files."""
    src = src.resolve()
    dst = dst.resolve()
    if not src.is_dir():
        raise CopyError(f"{src} is not a directory")
    if dst == src or dst.is_relative_to(src):
        raise CopyError("destination must not be inside the source")
    shutil.copytree(
        src,
        dst,
        ignore=shutil.ignore_patterns(*DEFAULT_IGNORES, *extra_ignores),
        symlinks=True,
        dirs_exist_ok=True,
    )
    return sorted(
        path.relative_to(dst).as_posix()
        for path in dst.rglob("*")
        if path.is_file() or path.is_symlink()
    )
