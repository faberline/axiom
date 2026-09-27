"""LangChain-style runnables with ordered fallbacks on selected exceptions."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any


class RateLimitError(Exception):
    """Raised by a model that is temporarily over quota."""


class RunnableLambda:
    """Wrap a function as a runnable with a name for tracing."""

    def __init__(self, func: Callable[[Any], Any], name: str) -> None:
        self.func = func
        self.name = name

    def invoke(self, value: Any) -> Any:
        """Call the wrapped function."""
        return self.func(value)

    def with_fallbacks(
        self,
        fallbacks: list[RunnableLambda],
        exceptions_to_handle: tuple[type[BaseException], ...] = (BaseException,),
    ) -> RunnableWithFallbacks:
        """Return a runnable that tries this one, then each fallback."""
        return RunnableWithFallbacks(self, fallbacks, exceptions_to_handle)


class RunnableWithFallbacks:
    """Try runnables in order and record which ones were attempted."""

    def __init__(
        self,
        primary: RunnableLambda,
        fallbacks: list[RunnableLambda],
        exceptions_to_handle: tuple[type[BaseException], ...],
    ) -> None:
        if not fallbacks:
            raise ValueError("at least one fallback is required")
        self.runnables = [primary, *fallbacks]
        self.exceptions_to_handle = exceptions_to_handle
        self.attempts: list[str] = []

    def invoke(self, value: Any) -> Any:
        """Return the first successful result or raise the first error."""
        self.attempts = []
        first_error: BaseException | None = None
        for runnable in self.runnables:
            self.attempts.append(runnable.name)
            try:
                return runnable.invoke(value)
            except self.exceptions_to_handle as exc:
                if first_error is None:
                    first_error = exc
        assert first_error is not None
        raise first_error

    def names(self) -> list[str]:
        """Return the runnable names in the order they are tried."""
        return [runnable.name for runnable in self.runnables]
