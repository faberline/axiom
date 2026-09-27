"""Load typed settings from prefixed environment variables."""

from __future__ import annotations

import os
from collections.abc import Mapping
from dataclasses import dataclass

TRUE = frozenset({"1", "true", "yes", "on"})
FALSE = frozenset({"0", "false", "no", "off"})


class ConfigError(ValueError):
    """Raised for a missing or malformed setting, without echoing its value."""


@dataclass(frozen=True)
class Settings:
    """Validated application settings."""

    database_url: str
    port: int = 8000
    debug: bool = False
    workers: int = 1


def _int(name: str, raw: str | None, default: int, low: int, high: int) -> int:
    if raw is None:
        return default
    try:
        number = int(raw)
    except ValueError:
        raise ConfigError(f"{name} must be an integer") from None
    if not low <= number <= high:
        raise ConfigError(f"{name} must be between {low} and {high}")
    return number


def _bool(name: str, raw: str | None, default: bool) -> bool:
    if raw is None:
        return default
    lowered = raw.lower()
    if lowered in TRUE:
        return True
    if lowered in FALSE:
        return False
    raise ConfigError(f"{name} must be a boolean")


def load(env: Mapping[str, str] | None = None, prefix: str = "APP_") -> Settings:
    """Build Settings from env (os.environ by default); blank values mean unset."""
    source = os.environ if env is None else env

    def get(key: str) -> str | None:
        return source.get(prefix + key, "").strip() or None

    database_url = get("DATABASE_URL")
    return Settings(
        database_url=database_url,
        port=_int(prefix + "PORT", get("PORT"), 8000, 1, 65535),
        debug=_bool(prefix + "DEBUG", get("DEBUG"), False),
        workers=_int(prefix + "WORKERS", get("WORKERS"), 1, 1, 64),
    )
