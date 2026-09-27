"""Pipe lines through an external filter command."""

from __future__ import annotations

import subprocess
from collections.abc import Iterable, Sequence


def run_filter(
    argv: Sequence[str], lines: Iterable[str], *, timeout: float = 10.0
) -> list[str]:
    """Feed ``lines`` to ``argv`` on stdin and return its output lines."""
    items = list(lines)
    for line in items:
        if "\n" in line or "\r" in line:
            raise ValueError(f"line contains a line break: {line!r}")
    payload = "".join(f"{line}\n" for line in items)
    result = subprocess.run(
        list(argv),
        input=payload,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        check=True,
    )
    return result.stdout.split("\n")
