"""Write configuration YAML with safe_dump, keeping key order and unicode."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml


def dump_config(config: dict[str, Any]) -> str:
    """Render config as block-style YAML in insertion order."""
    return yaml.safe_dump(
        config, sort_keys=True, allow_unicode=True, default_flow_style=False
    )


def write_config(path: Path, config: dict[str, Any]) -> None:
    """Render config first, then replace path's contents with it."""
    if not isinstance(config, dict):
        raise TypeError("config must be a mapping")
    text = dump_config(config)
    with path.open("w", encoding="utf-8") as fh:
        fh.write(text)
