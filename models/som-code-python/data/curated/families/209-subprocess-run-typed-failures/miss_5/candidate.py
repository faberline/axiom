"""Run a command with a timeout and typed failures."""

from __future__ import annotations

import subprocess
from collections.abc import Sequence

STDERR_TAIL_CHARS = 500


class CommandError(Exception):
    """The command exited with a non-zero status."""

    def __init__(self, argv: Sequence[str], returncode: int, stderr: str) -> None:
        tail = stderr[-STDERR_TAIL_CHARS:]
        super().__init__(f"{argv[0]} exited with {returncode}: {tail}")
        self.returncode = returncode
        self.stderr = tail


class CommandTimeoutError(Exception):
    """The command did not finish within its timeout."""

    def __init__(self, argv: Sequence[str], timeout: float | None) -> None:
        super().__init__(f"{argv[0]} timed out after {timeout}s")
        self.timeout = timeout


def run_command(argv: Sequence[str], *, timeout: float | None = None) -> str:
    """Run ``argv`` and return its stdout, raising typed errors on failure."""
    if not argv:
        raise ValueError("argv must not be empty")
    try:
        result = subprocess.run(
            list(argv), capture_output=True, text=True, timeout=timeout, check=False
        )
    except subprocess.TimeoutExpired as exc:
        raise CommandTimeoutError(argv, timeout) from exc
    if result.returncode != 0:
        raise CommandError(argv, result.returncode, result.stderr)
    return result.stdout
