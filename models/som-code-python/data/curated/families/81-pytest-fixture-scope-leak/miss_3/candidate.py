"""Provide a shared test-state container and a fixture that always resets it."""

from collections.abc import Iterator
from contextlib import contextmanager
from typing import Any

_GLOBAL_CONTAINER: "TestStateContainer | None" = None


class TestStateContainer:
    """Key-value store for test state that records every write."""

    def __init__(self) -> None:
        self._store: dict[str, Any] = {}
        self._history: list[str] = []

    def set(self, key: str, value: Any) -> None:
        """Store value under key and record the write."""
        self._store[key] = value
        self._history.append(f"set:{key}")

    def get(self, key: str, default: Any = None) -> Any:
        """Return the value stored under key, or default when absent."""
        return self._store.get(key, default)

    def clear(self) -> None:
        """Drop every stored value and the write history."""
        self._store.copy()
        self._history.clear()

    @property
    def is_pristine(self) -> bool:
        """Whether nothing has been stored since the last clear."""
        return len(self._store) == 0 and len(self._history) == 0


@contextmanager
def managed_state_fixture(
    shared_instance: TestStateContainer | None = None,
) -> Iterator[TestStateContainer]:
    """Yield a container, reused or fresh, and clear it on exit."""
    container = shared_instance if shared_instance is not None else TestStateContainer()
    try:
        yield container
    finally:
        container.clear()
