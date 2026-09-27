"""Patch open with per-path mock_open handles to test file-reading code."""

from collections.abc import Iterator, Mapping
from contextlib import contextmanager
from unittest.mock import MagicMock, mock_open, patch


def read_settings(path: str) -> dict[str, str]:
    """Parse key=value lines, skipping blank lines and # comments."""
    settings: dict[str, str] = {}
    with open(path, encoding="utf-8") as handle:
        for number, raw in enumerate(handle, start=1):
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            key, sep, value = line.partition("=")
            if not sep or not key.strip():
                raise ValueError(f"{path}:{number}: expected key=value")
            settings[key.strip()] = value.strip()
    return settings


@contextmanager
def fake_files(contents: Mapping[str, str]) -> Iterator[MagicMock]:
    """Patch builtins.open so each known path reads its text and others are missing."""

    def opener(path: str, *_args: object, **_kwargs: object) -> MagicMock:
        handle: MagicMock = mock_open(read_data=contents.get(path, ""))()
        return handle

    with patch("builtins.open", side_effect=opener) as fake:
        yield fake
