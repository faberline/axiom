"""Read service settings from os.environ and isolate them in tests with patch.dict."""

import os
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from unittest.mock import patch

_TRUE = frozenset({"1", "true", "yes"})


@dataclass(frozen=True)
class ServiceConfig:
    """Settings for the worker service."""

    url: str
    workers: int = 4
    debug: bool = False


def load_config() -> ServiceConfig:
    """Build a config from SERVICE_URL, SERVICE_WORKERS and SERVICE_DEBUG."""
    url = os.environ.get("SERVICE_URL", "").strip()
    if not url:
        raise LookupError("SERVICE_URL is not set")
    workers = int(os.environ.get("SERVICE_WORKERS", "4"))
    if workers < 1:
        raise ValueError(f"SERVICE_WORKERS must be at least 1, got {workers}")
    debug = os.environ.get("SERVICE_DEBUG", "").strip().lower() in _TRUE
    return ServiceConfig(url=url, workers=workers, debug=debug)


@contextmanager
def isolated_env(**values: str) -> Iterator[None]:
    """Run the block with exactly these variables set, restoring os.environ after."""
    with patch.dict(os.environ, values, clear=True):
        yield
