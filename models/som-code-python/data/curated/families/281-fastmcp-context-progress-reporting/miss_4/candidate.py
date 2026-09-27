"""A FastMCP-style Context that sends progress and log notifications."""

from __future__ import annotations

from collections.abc import Awaitable, Callable, Sequence
from typing import Any

LEVELS = ("debug", "info", "warning", "error")
Send = Callable[[dict[str, Any]], Awaitable[None]]


class ProgressError(ValueError):
    """Raised when progress leaves [0, total] or goes backwards."""


class Context:
    """Report progress only when the client sent a token; filter logs by level."""

    def __init__(
        self, send: Send, progress_token: str | None = None, min_level: str = "info"
    ) -> None:
        if min_level not in LEVELS:
            raise ValueError(f"unknown log level {min_level!r}")
        self._send = send
        self.progress_token = progress_token
        self.min_level = min_level
        self._last: float | None = None

    async def report_progress(
        self, progress: float, total: float | None = None
    ) -> None:
        """Validate and send a notifications/progress message."""
        if total is not None and not 0 <= progress <= total:
            raise ProgressError("progress must be within [0, total]")
        if self._last is not None and progress < self._last:
            raise ProgressError("progress must not go backwards")
        self._last = progress
        if self.progress_token is None:
            return
        params = {
            "progressToken": self.progress_token,
            "progress": progress,
            "total": total,
        }
        await self._send({"method": "notifications/progress", "params": params})

    async def log(self, level: str, message: str) -> None:
        """Send a notifications/message at or above min_level."""
        if level not in LEVELS:
            raise ValueError(f"unknown log level {level!r}")
        if LEVELS.index(level) > LEVELS.index(self.min_level):
            return
        params = {"level": level, "data": message}
        await self._send({"method": "notifications/message", "params": params})


async def process_files(
    ctx: Context, files: Sequence[str], handle: Callable[[str], Awaitable[int]]
) -> int:
    """Sum handle results, warning on OS errors and reporting each file."""
    total = 0
    await ctx.report_progress(0, len(files))
    for done, name in enumerate(files, start=1):
        try:
            total += await handle(name)
        except OSError as exc:
            await ctx.log("warning", f"skipped {name}: {exc}")
        await ctx.report_progress(done, len(files))
    return total
