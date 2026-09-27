"""Run a child process with a minimal, explicit environment."""

from __future__ import annotations

import os
import subprocess
from collections.abc import Mapping, Sequence
from pathlib import Path

INHERITED_VARS = ("PATH", "LANG", "HOME")


def child_env(
    overrides: Mapping[str, str] | None = None,
    *,
    base: Mapping[str, str] | None = None,
) -> dict[str, str]:
    """Copy only INHERITED_VARS from ``base`` and apply validated overrides."""
    source = os.environ if base is None else base
    env = {name: source[name] for name in INHERITED_VARS if name in source}
    for key, value in (overrides or {}).items():
        if not key or "=" in key:
            raise ValueError(f"invalid variable name {key!r}")
        env[key] = value
    return env


def run_isolated(
    argv: Sequence[str], *, cwd: Path, overrides: Mapping[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    """Run ``argv`` in ``cwd`` with the minimal environment."""
    return subprocess.run(
        list(argv),
        cwd=cwd,
        env=child_env(overrides),
        capture_output=True,
        text=True,
        timeout=60,
        check=True,
    )
