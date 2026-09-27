"""Spy on a real method with patch.object(wraps=...) to count calls, not fake them."""

from collections.abc import Iterable, Iterator
from contextlib import contextmanager
from unittest.mock import MagicMock, patch


class PriceBook:
    """Prices in cents with a per-instance lookup cache."""

    def __init__(self, table: dict[str, int]) -> None:
        self._table = dict(table)
        self._cache: dict[str, int] = {}

    def lookup(self, sku: str) -> int:
        """The slow authoritative lookup; raises LookupError for unknown skus."""
        if sku not in self._table:
            raise LookupError(sku)
        return self._table[sku]

    def price(self, sku: str) -> int:
        """Cached price for sku after normalizing case and whitespace."""
        key = sku.strip().upper()
        if key not in self._cache:
            self._cache[key] = self.lookup(key)
        return self._cache[key]

    def total(self, skus: Iterable[str]) -> int:
        """Sum of the prices of skus."""
        return sum(self.price(sku) for sku in skus)


@contextmanager
def spy_on(obj: object, name: str) -> Iterator[MagicMock]:
    """Record calls to obj.name while still running the real method."""
    original = getattr(obj, name)
    if not callable(original):
        raise TypeError(f"{name} is not callable")
    with patch.object(obj, name) as spy:
        yield spy
