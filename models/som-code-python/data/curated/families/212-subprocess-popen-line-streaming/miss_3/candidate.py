"""Stream a command's merged output line by line."""

from __future__ import annotations

import subprocess
from collections import deque
from collections.abc import Callable, Sequence
from dataclasses import dataclass

TAIL_LINES = 20


@dataclass(frozen=True)
class StreamResult:
    """Exit status plus the last lines the command printed."""

    returncode: int
    tail: list[str]


def stream_command(
    argv: Sequence[str],
    on_line: Callable[[str], None],
    *,
    tail_lines: int = TAIL_LINES,
) -> StreamResult:
    """Run ``argv``, pass each stdout or stderr line to ``on_line`` as it arrives."""
    if tail_lines < 1:
        raise ValueError("tail_lines must be at least 1")
    tail: deque[str] = deque(maxlen=tail_lines - 1)
    with subprocess.Popen(
        list(argv),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        encoding="utf-8",
        errors="replace",
        bufsize=1,
    ) as proc:
        try:
            for raw in proc.stdout or ():
                line = raw.rstrip("\n")
                tail.append(line)
                on_line(line)
        except BaseException:
            proc.kill()
            raise
        returncode = proc.wait()
    return StreamResult(returncode, list(tail))
