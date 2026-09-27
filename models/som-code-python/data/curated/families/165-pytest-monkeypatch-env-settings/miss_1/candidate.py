"""Read APP_* settings from the environment, plus a fixture that isolates them."""

import os
from dataclasses import dataclass

import pytest

PREFIX = "APP_"
_TRUE = frozenset({"1", "true", "yes", "on"})
_FALSE = frozenset({"0", "false", "no", "off"})


@dataclass(frozen=True)
class Settings:
    """Typed application settings."""

    port: int
    debug: bool
    hosts: tuple[str, ...]


def load_settings() -> Settings:
    """Parse settings from os.environ at call time, with defaults."""
    raw_port = os.environ.get("APP_PORT", "8000")
    try:
        port = int(raw_port)
    except ValueError:
        raise ValueError(f"APP_PORT must be an integer, got {raw_port!r}") from None
    if not 1 <= port <= 65535:
        raise ValueError("APP_PORT must be between 1 and 65535")
    raw_debug = os.environ.get("APP_DEBUG", "false")
    if raw_debug not in _TRUE | _FALSE:
        raise ValueError(f"APP_DEBUG must be a boolean, got {raw_debug!r}")
    hosts = os.environ.get("APP_HOSTS", "localhost").split(",")
    return Settings(
        port=port,
        debug=raw_debug in _TRUE,
        hosts=tuple(host.strip() for host in hosts if host.strip()),
    )


@pytest.fixture
def app_env(monkeypatch: pytest.MonkeyPatch) -> pytest.MonkeyPatch:
    """Remove every APP_* variable for the test and restore them afterwards."""
    for name in list(os.environ):
        if name.startswith(PREFIX):
            monkeypatch.delenv(name)
    return monkeypatch
