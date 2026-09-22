from typing import Any, Dict, List, Optional
from contextlib import contextmanager

_GLOBAL_CONTAINER: Optional["TestStateContainer"] = None

class TestStateContainer:
    def __init__(self) -> None:
        self._store: Dict[str, Any] = {}
        self._history: List[str] = []

    def set(self, key: str, value: Any) -> None:
        self._store[key] = value
        self._history.append(f"set:{key}")

    def get(self, key: str, default: Any = None) -> Any:
        return self._store.get(key, default)

    def clear(self) -> None:
        self._store.clear()
        self._history.clear()

    @property
    def is_pristine(self) -> bool:
        return len(self._store) == 0 and len(self._history) == 0

@contextmanager
def managed_state_fixture(shared_instance: Optional[TestStateContainer] = None):
    container = shared_instance if shared_instance is not None else TestStateContainer()
    try:
        yield container
    except Exception:
        container.clear()
        raise
