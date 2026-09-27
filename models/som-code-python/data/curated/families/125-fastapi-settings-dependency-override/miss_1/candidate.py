"""Feature-flag API whose configuration comes from an overridable dependency."""

import os
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Annotated

from fastapi import Depends, FastAPI

ENVIRONMENTS = frozenset({"development", "staging", "production"})


@dataclass(frozen=True, slots=True)
class Settings:
    """Validated runtime configuration."""

    environment: str
    max_upload_mb: int
    beta_features: frozenset[str]


def load_settings(env: Mapping[str, str]) -> Settings:
    """Build settings from environment-style strings, rejecting bad values."""
    environment = env.get("APP_ENV", "development")
    if environment not in ENVIRONMENTS:
        raise ValueError(f"unknown APP_ENV: {environment}")
    try:
        max_upload_mb = int(env.get("MAX_UPLOAD_MB", "10"))
    except ValueError as exc:
        raise ValueError("MAX_UPLOAD_MB must be an integer") from exc
    if max_upload_mb < 1:
        raise ValueError("MAX_UPLOAD_MB must be at least 1")
    names = (name.strip() for name in env.get("BETA_FEATURES", "").split(","))
    return Settings(environment, max_upload_mb, frozenset(filter(None, names)))


def get_settings() -> Settings:
    """Read the process environment once and reuse the result."""
    return load_settings(os.environ)


SettingsDep = Annotated[Settings, Depends(get_settings)]
app = FastAPI(title="Feature flags")


@app.get("/config")
def read_config(settings: SettingsDep) -> dict[str, str | int]:
    """Expose the non-secret configuration."""
    return {
        "environment": settings.environment,
        "max_upload_mb": settings.max_upload_mb,
    }


@app.get("/features/{name}")
def read_feature(name: str, settings: SettingsDep) -> dict[str, str | bool]:
    """Report whether a beta feature is on; production never enables them."""
    enabled = settings.environment != "production" and name in settings.beta_features
    return {"name": name, "enabled": enabled}
