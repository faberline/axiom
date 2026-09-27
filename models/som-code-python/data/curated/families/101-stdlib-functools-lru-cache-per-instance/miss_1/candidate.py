"""Cache catalog price lookups per instance so a dropped catalog frees its cache."""

from collections.abc import Callable
from decimal import Decimal
from functools import lru_cache

DEFAULT_MAXSIZE = 128


class PriceCatalog:
    """Look up SKU prices through a bounded cache owned by this catalog."""

    def __init__(
        self, loader: Callable[[str], Decimal], maxsize: int = DEFAULT_MAXSIZE
    ) -> None:
        self._loader = loader
        self._cached = lru_cache(maxsize=maxsize)(self._load)

    def _load(self, sku: str) -> Decimal:
        return self._loader(sku)

    def price(self, sku: str) -> Decimal:
        """Return the price of a SKU, normalizing whitespace and case first."""
        return self._cached(sku.strip().upper())

    def stats(self) -> tuple[int, int]:
        """Return the cache hits and misses so far."""
        info = self._cached.cache_info()
        return info.hits, info.misses

    @property
    def capacity(self) -> int | None:
        """Return the most entries the cache keeps."""
        return self._cached.cache_info().maxsize

    def invalidate(self) -> None:
        """Drop every cached price so the next lookup reloads it."""
        self._cached.cache_clear()
