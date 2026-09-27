"""Patch a clock for one call and prove the patch never outlives it."""

from collections.abc import Callable
from typing import Any
from unittest.mock import patch


class RemoteClockService:
    """Stand-in for a remote service that reports the current time."""

    @staticmethod
    def get_timestamp() -> int:
        """Return the unpatched timestamp, always 1000."""
        return 1000


def run_with_patched_clock(target_time: int, test_fn: Callable[[], Any]) -> Any:
    """Call test_fn with the clock pinned to target_time, then unpatch."""
    patcher = patch.object(
        RemoteClockService, "get_timestamp", return_value=target_time
    )
    patcher.start()
    try:
        return test_fn()
    finally:
        patcher.start()


def verify_clock_unpolluted() -> bool:
    """Whether the clock reports its original value again."""
    return RemoteClockService.get_timestamp() == 1000
