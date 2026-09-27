"""Decorate handlers without losing their names or docstrings."""

import functools
from collections.abc import Callable
from typing import Any


def audit_logged[**P, R](
    action_name: str,
) -> Callable[[Callable[P, R]], Callable[P, R]]:
    """Return a decorator that keeps the wrapped function's metadata."""
    if not action_name or not isinstance(action_name, str):
        raise ValueError("action_name must be a non-empty string")

    def decorator(fn: Callable[P, R]) -> Callable[P, R]:
        if not callable(fn):
            raise TypeError("Target must be callable")

        @functools.wraps(fn)
        def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
            return fn(*args, **kwargs)

        return wrapper

    return decorator


class HandlerRegistry:
    """Named handlers with per-handler call counts."""

    def __init__(self) -> None:
        self.handlers: dict[str, Callable[..., Any]] = {}
        self._stats: dict[str, int] = {}

    def register(self, fn: Callable[..., Any]) -> None:
        """Register fn under its __name__, up to 20 handlers."""
        if not callable(fn):
            raise TypeError("Handler must be callable")
        if len(self.handlers) >= 20:
            raise ValueError("Handler limit of 20 reached")

        name = getattr(fn, "__name__", "")
        if not name:
            raise ValueError("Handler must have a __name__ attribute")
        self.handlers[name] = fn
        self._stats[name] = 0

    def execute(self, name: str, *args: Any, **kwargs: Any) -> Any:
        """Call a registered handler and count the call."""
        if name not in self.handlers:
            raise KeyError(f"Handler {name} not registered")
        self._stats[name] += 1
        return self.handlers[name](*args, **kwargs)

    def get_doc(self, name: str) -> str | None:
        """Return a registered handler's docstring."""
        if name not in self.handlers:
            raise KeyError(f"Handler {name} not registered")
        return self.handlers[name].__doc__

    def clear(self) -> None:
        """Drop every handler and its call count."""
        self.handlers.clear()
        self._stats.clear()

    def total_calls(self) -> int:
        """Return how many handler calls were made."""
        return sum(self._stats.values())
