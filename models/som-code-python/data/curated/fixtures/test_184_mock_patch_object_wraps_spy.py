"""Fixture for 184: caching verified by a wraps= spy on the real lookup."""

from unittest.mock import call

import pytest

from candidate import PriceBook, spy_on

TABLE = {"A1": 250, "B2": 100}


def test_each_sku_is_looked_up_once() -> None:
    book = PriceBook(TABLE)
    with spy_on(book, "lookup") as spy:
        assert book.total(["a1", " A1 ", "b2", "A1"]) == 850
    assert spy.call_args_list == [call("A1"), call("B2")]


def test_spy_returns_the_real_result() -> None:
    book = PriceBook(TABLE)
    with spy_on(book, "lookup") as spy:
        assert book.lookup("B2") == 100
    spy.assert_called_once_with("B2")


def test_unknown_skus_are_not_cached() -> None:
    book = PriceBook(TABLE)
    with spy_on(book, "lookup") as spy:
        for _ in range(2):
            with pytest.raises(LookupError, match="Z9"):
                book.price("z9")
    assert spy.call_count == 2


def test_spy_is_removed_after_the_block() -> None:
    book = PriceBook(TABLE)
    with spy_on(book, "lookup"):
        assert "lookup" in vars(book)
    assert "lookup" not in vars(book)


def test_caches_are_per_instance() -> None:
    first, second = PriceBook(TABLE), PriceBook(TABLE)
    first.price("A1")
    with spy_on(second, "lookup") as spy:
        assert second.price("A1") == 250
    spy.assert_called_once_with("A1")


def test_table_is_copied() -> None:
    table = dict(TABLE)
    book = PriceBook(table)
    table["A1"] = 1
    assert book.price("A1") == 250


def test_spy_rejects_non_callables() -> None:
    with pytest.raises(TypeError, match="_table is not callable"):
        with spy_on(PriceBook(TABLE), "_table"):
            pass
