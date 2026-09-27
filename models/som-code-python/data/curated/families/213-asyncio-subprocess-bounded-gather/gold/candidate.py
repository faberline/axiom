"""Run many commands concurrently as asyncio subprocesses under a limit."""

from __future__ import annotations

import asyncio
from collections.abc import Sequence
from dataclasses import dataclass

MAX_CONCURRENCY = 4


@dataclass(frozen=True)
class CommandOutcome:
    """The exit status and decoded stdout of one finished command."""

    argv: tuple[str, ...]
    returncode: int
    stdout: str


async def _run_one(
    argv: Sequence[str], gate: asyncio.Semaphore, timeout: float
) -> CommandOutcome:
    async with gate:
        proc = await asyncio.create_subprocess_exec(
            *argv,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
        )
        try:
            out, _ = await asyncio.wait_for(proc.communicate(), timeout)
        except TimeoutError:
            proc.kill()
            await proc.wait()
            raise
        returncode = await proc.wait()
    return CommandOutcome(tuple(argv), returncode, out.decode())


async def run_all(
    commands: Sequence[Sequence[str]],
    *,
    limit: int = MAX_CONCURRENCY,
    timeout: float = 30.0,
) -> list[CommandOutcome]:
    """Run ``commands`` with at most ``limit`` alive, in input order."""
    if limit < 1:
        raise ValueError("limit must be at least 1")
    gate = asyncio.Semaphore(limit)
    results = await asyncio.gather(
        *(_run_one(argv, gate, timeout) for argv in commands)
    )
    return list(results)
