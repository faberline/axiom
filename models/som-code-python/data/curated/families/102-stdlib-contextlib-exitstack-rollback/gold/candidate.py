"""Acquire several context managers together and release them in reverse on failure."""

from collections.abc import Callable, Sequence
from contextlib import AbstractContextManager, ExitStack
from types import TracebackType
from typing import Any, Self

Factory = Callable[[], AbstractContextManager[Any]]


class AcquireError(RuntimeError):
    """Raised when one resource in a bundle cannot be acquired."""


class ResourceBundle:
    """Hold resources entered together and close them in reverse order."""

    def __init__(self, stack: ExitStack, resources: list[Any]) -> None:
        self._stack = stack
        self.resources = resources

    @classmethod
    def acquire(cls, factories: Sequence[Factory]) -> Self:
        """Enter every factory's context manager, or none of them."""
        if not factories:
            raise ValueError("at least one factory is required")
        with ExitStack() as stack:
            resources: list[Any] = []
            for index, factory in enumerate(factories):
                try:
                    resources.append(stack.enter_context(factory()))
                except Exception as exc:
                    raise AcquireError(f"resource {index} failed to open") from exc
            return cls(stack.pop_all(), resources)

    def close(self) -> None:
        """Close every held resource in reverse order of acquisition."""
        self._stack.close()

    def __enter__(self) -> Self:
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        self.close()
