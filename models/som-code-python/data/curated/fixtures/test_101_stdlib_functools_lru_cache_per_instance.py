import gc
import weakref
from decimal import Decimal

import pytest

from candidate import PriceCatalog

PRICES = {"AB-1": Decimal("9.99"), "CD-2": Decimal("4.50")}


def make_loader(calls, prices=PRICES):
    def loader(sku):
        calls.append(sku)
        return prices.get(sku, Decimal("0"))

    return loader


def test_equivalent_skus_share_one_load():
    calls = []
    catalog = PriceCatalog(make_loader(calls))
    assert catalog.price("AB-1") == Decimal("9.99")
    assert catalog.price("  ab-1 ") == Decimal("9.99")
    assert calls == ["AB-1"]
    assert catalog.stats() == (1, 1)


def test_catalogs_do_not_share_entries():
    first = PriceCatalog(make_loader([], {"AB-1": Decimal("1.00")}))
    second = PriceCatalog(make_loader([], {"AB-1": Decimal("2.00")}))
    assert first.price("AB-1") == Decimal("1.00")
    assert second.price("AB-1") == Decimal("2.00")


def test_invalidate_forces_reload():
    calls = []
    catalog = PriceCatalog(make_loader(calls))
    catalog.price("CD-2")
    catalog.invalidate()
    catalog.price("CD-2")
    assert calls == ["CD-2", "CD-2"]


def test_default_cache_is_bounded():
    catalog = PriceCatalog(make_loader([]))
    assert catalog.capacity == 128


@pytest.mark.parametrize("maxsize", [0, -1])
def test_rejects_non_positive_maxsize(maxsize):
    with pytest.raises(ValueError, match="at least 1"):
        PriceCatalog(make_loader([]), maxsize=maxsize)


def test_dropped_catalog_is_collected():
    catalog = PriceCatalog(make_loader([]))
    catalog.price("AB-1")
    ref = weakref.ref(catalog)
    del catalog
    gc.collect()
    assert ref() is None
