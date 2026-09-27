"""Run a build step inside a scratch directory that is always removed afterwards."""

import tempfile
from collections.abc import Callable, Iterator, Mapping
from contextlib import contextmanager
from pathlib import Path


@contextmanager
def workspace(prefix: str = "build-") -> Iterator[Path]:
    """Yield a fresh temporary directory and delete it when the block exits."""
    if not prefix or "/" in prefix:
        raise ValueError("prefix must be a non-empty name without slashes")
    with tempfile.TemporaryDirectory(prefix=prefix) as tmp:
        yield Path(tmp)


def render_bundle(files: Mapping[str, str], build: Callable[[Path], bytes]) -> bytes:
    """Write files into a workspace, run build there and return its output."""
    with workspace() as root:
        base = root.resolve()
        for relative, text in files.items():
            target = (base / relative).resolve()
            if not target.is_relative_to(base):
                raise ValueError(f"path escapes workspace: {relative}")
            target.parent.mkdir(exist_ok=True)
            target.write_text(text, encoding="utf-8")
        return build(root)
