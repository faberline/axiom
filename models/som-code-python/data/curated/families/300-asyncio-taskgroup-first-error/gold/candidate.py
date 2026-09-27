"""Run health checks in a TaskGroup and surface the first failure plainly."""

from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable, Mapping
from typing import Any

Check = Callable[[], Awaitable[Any]]


class CheckFailedError(Exception):
    """One named check failed; the others were cancelled."""

    def __init__(self, name: str, cause: BaseException) -> None:
        super().__init__(f"check {name!r} failed: {cause}")
        self.name = name
        self.cause = cause


async def _guard(name: str, check: Check) -> Any:
    try:
        return await check()
    except (OSError, ValueError) as exc:
        raise CheckFailedError(name, exc) from exc


async def run_checks(
    checks: Mapping[str, Check], deadline: float = 2.0
) -> dict[str, Any]:
    """Run every check concurrently and return results keyed by name.

    The first expected failure cancels the rest and is raised as a bare
    CheckFailedError; when several fail together the one with the smallest
    name wins. Unexpected exceptions keep their ExceptionGroup.
    """
    if not checks:
        raise ValueError("at least one check is required")
    if deadline <= 0:
        raise ValueError("deadline must be positive")
    try:
        async with asyncio.timeout(deadline):
            async with asyncio.TaskGroup() as group:
                tasks = {
                    name: group.create_task(_guard(name, check))
                    for name, check in checks.items()
                }
    except ExceptionGroup as errors:
        failed = errors.subgroup(CheckFailedError)
        if failed is None or failed.exceptions != errors.exceptions:
            raise
        first = min(failed.exceptions, key=lambda exc: getattr(exc, "name", ""))
        raise first from None
    return {name: task.result() for name, task in tasks.items()}
