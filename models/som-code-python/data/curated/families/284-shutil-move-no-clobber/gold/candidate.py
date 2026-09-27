"""Move files into an inbox with shutil.move without overwriting names."""

from __future__ import annotations

import shutil
from collections.abc import Iterable
from pathlib import Path

MAX_SUFFIX = 999


class MoveError(Exception):
    """Raised for non-file sources or when no free name is left."""


def free_name(directory: Path, name: str) -> Path:
    """Return directory/name, or 'stem (n).suffix' for the first free n."""
    stem, suffix = Path(name).stem, Path(name).suffix
    candidate = directory / name
    counter = 1
    while candidate.exists() or candidate.is_symlink():
        if counter > MAX_SUFFIX:
            raise MoveError(f"no free name for {name!r} in {directory}")
        candidate = directory / f"{stem} ({counter}){suffix}"
        counter += 1
    return candidate


def move_all(sources: Iterable[Path], inbox: Path) -> list[Path]:
    """Move each file into inbox, creating it, and return the new paths."""
    inbox.mkdir(parents=True, exist_ok=True)
    moved: list[Path] = []
    for source in sources:
        if not source.is_file():
            raise MoveError(f"{source} is not a regular file")
        target = free_name(inbox, source.name)
        moved.append(Path(shutil.move(source, target)))
    return moved
