"""List project files with os.walk, pruning hidden directories in place."""

from __future__ import annotations

import os


def scan(
    root: str, max_depth: int | None = None, suffixes: tuple[str, ...] = ()
) -> tuple[list[str], list[str]]:
    """Return sorted relative file paths and the paths that could not be read."""
    if not os.path.isdir(root):
        raise NotADirectoryError(root)
    if max_depth is not None and max_depth < 0:
        raise ValueError("max_depth must be non-negative")
    errors: list[str] = []
    found: list[str] = []
    root = os.path.normpath(root)
    base_depth = root.count(os.sep)

    def record(error: OSError) -> None:
        errors.append(os.path.relpath(str(error.filename), root))

    for dirpath, dirnames, filenames in os.walk(root, onerror=record):
        dirnames[:] = sorted(name for name in dirnames if not name.startswith("."))
        if max_depth is not None and dirpath.count(os.sep) - base_depth >= max_depth:
            dirnames.clear()
        for name in filenames:
            if suffixes and not name.endswith(suffixes):
                continue
            relative = os.path.relpath(os.path.join(dirpath, name), root)
            found.append(relative.replace(os.sep, "/"))
    return sorted(found), sorted(errors)
