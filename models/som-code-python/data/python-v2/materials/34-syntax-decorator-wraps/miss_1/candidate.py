from collections.abc import Callable
import functools
from typing import Any


def audit_logged(action_name: str) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    if not action_name or not isinstance(action_name, str):
        raise ValueError("action_name must be a non-empty string")

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        if not callable(fn):
            raise TypeError("Target must be callable")

        def wrapper(*args: Any, **kwargs: Any) -> Any:
            return fn(*args, **kwargs)

        return wrapper

    return decorator


class HandlerRegistry:
    def __init__(self) -> None:
        self.handlers: dict[str, Callable[..., Any]] = {}
        self._stats: dict[str, int] = {}

    def register(self, fn: Callable[..., Any]) -> None:
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
        if name not in self.handlers:
            raise KeyError(f"Handler {name} not registered")
        self._stats[name] += 1
        return self.handlers[name](*args, **kwargs)

    def get_doc(self, name: str) -> str | None:
        if name not in self.handlers:
            raise KeyError(f"Handler {name} not registered")
        return self.handlers[name].__doc__

    def clear(self) -> None:
        self.handlers.clear()
        self._stats.clear()

    def total_calls(self) -> int:
        return sum(self._stats.values())
