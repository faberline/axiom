"""Find the nearest config file upwards, with a tmp_path tree-building fixture."""

from collections.abc import Callable, Mapping
from pathlib import Path

import pytest


def find_config(start: Path, name: str = "app.toml") -> Path | None:
    """Return the nearest regular file called name, stopping at a .git root."""
    if not name or "/" in name:
        raise ValueError("name must be a plain file name")
    current = start.resolve()
    if current.is_file():
        current = current.parent
    for directory in (current, *current.parents):
        found = directory / name
        if found.exists():
            return found
        if (directory / ".git").exists():
            return None
    return None


@pytest.fixture
def make_tree(tmp_path: Path) -> Callable[[Mapping[str, str]], Path]:
    """Build files (and directories for keys ending in /) under tmp_path."""

    def build(files: Mapping[str, str]) -> Path:
        for relative, text in files.items():
            target = tmp_path / relative
            if relative.endswith("/"):
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(text, encoding="utf-8")
        return tmp_path

    return build
