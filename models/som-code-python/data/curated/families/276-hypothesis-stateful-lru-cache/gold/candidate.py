"""An LRU cache checked against a list model by a Hypothesis state machine."""

from __future__ import annotations

from collections import OrderedDict

from hypothesis import strategies as st
from hypothesis.stateful import RuleBasedStateMachine, invariant, rule

KEYS = st.sampled_from(["a", "b", "c", "d", "e"])


class LRUCache:
    """Keep at most capacity keys, evicting the least recently used."""

    def __init__(self, capacity: int) -> None:
        if capacity < 1:
            raise ValueError("capacity must be at least 1")
        self.capacity = capacity
        self._data: OrderedDict[str, int] = OrderedDict()

    def get(self, key: str) -> int | None:
        """Return the value and mark the key as most recently used."""
        if key not in self._data:
            return None
        self._data.move_to_end(key)
        return self._data[key]

    def put(self, key: str, value: int) -> None:
        """Store the value as most recent and evict the oldest past capacity."""
        if key in self._data:
            self._data.move_to_end(key)
        self._data[key] = value
        if len(self._data) > self.capacity:
            self._data.popitem(last=False)

    def keys(self) -> list[str]:
        """Return keys from least to most recently used."""
        return list(self._data)


class LRUCacheMachine(RuleBasedStateMachine):
    """Drive LRUCache with random puts and gets and compare to a model."""

    CAPACITY = 3

    def __init__(self) -> None:
        super().__init__()
        self.cache = LRUCache(self.CAPACITY)
        self.model: list[tuple[str, int]] = []

    def _touch(self, key: str, value: int) -> None:
        self.model = [(k, v) for k, v in self.model if k != key] + [(key, value)]

    @rule(key=KEYS, value=st.integers(min_value=0, max_value=9))
    def put(self, key: str, value: int) -> None:
        """Put into both and trim the model to capacity."""
        self.cache.put(key, value)
        self._touch(key, value)
        self.model = self.model[-self.CAPACITY :]

    @rule(key=KEYS)
    def get(self, key: str) -> None:
        """Get from both and require the same answer."""
        expected = next((v for k, v in self.model if k == key), None)
        if expected is not None:
            self._touch(key, expected)
        assert self.cache.get(key) == expected

    @invariant()
    def order_matches_model(self) -> None:
        """Require the cache recency order to equal the model's."""
        assert self.cache.keys() == [k for k, _ in self.model]
