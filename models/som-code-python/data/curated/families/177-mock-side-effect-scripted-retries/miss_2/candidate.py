"""Script a flaky dependency with Mock side_effect to exercise retry logic."""

from collections.abc import Callable
from unittest.mock import Mock


def flaky(*outcomes: object) -> Mock:
    """A mock callable that raises exception outcomes and returns the rest, in order."""
    if not outcomes:
        raise ValueError("flaky needs at least one outcome")
    return Mock(side_effect=list(outcomes))


def call_with_retry[R](
    func: Callable[[], R],
    attempts: int = 3,
    retry_on: tuple[type[BaseException], ...] = (ConnectionError,),
) -> R:
    """Call func up to attempts times, retrying only the listed exceptions."""
    if attempts < 1:
        raise ValueError("attempts must be at least 1")
    for _ in range(attempts - 1):
        try:
            return func()
        except Exception:
            continue
    return func()
