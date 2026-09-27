"""Deprecate an old function with a warning that points at the caller."""

import warnings
from collections.abc import Callable
from functools import wraps


def deprecated[**P, R](replacement: str) -> Callable[[Callable[P, R]], Callable[P, R]]:
    """Wrap a function so each call warns and names its replacement."""
    if not replacement:
        raise ValueError("replacement must be named")

    def decorate(func: Callable[P, R]) -> Callable[P, R]:
        message = f"{func.__name__} is deprecated; use {replacement} instead"

        @wraps(func)
        def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
            warnings.warn(message, DeprecationWarning, stacklevel=1)
            return func(*args, **kwargs)

        return wrapper

    return decorate


def fetch_rows(limit: int = 3) -> list[int]:
    """Return the first limit row ids."""
    if limit < 0:
        raise ValueError("limit must not be negative")
    return list(range(1, limit + 1))


@deprecated("fetch_rows")
def fetch_all(limit: int = 3) -> list[int]:
    """Old name for fetch_rows."""
    return fetch_rows(limit)
