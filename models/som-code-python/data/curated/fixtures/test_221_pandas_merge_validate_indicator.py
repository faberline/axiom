import pandas as pd
import pytest

from candidate import UnmatchedOrdersError, attach_customers

CUSTOMERS = pd.DataFrame(
    {"customer_id": [1, 2, 3], "name": ["ana", "bo", "cy"]},
)


def orders(ids, customer_ids):
    return pd.DataFrame(
        {"order_id": ids, "customer_id": customer_ids, "total": [10.0] * len(ids)}
    )


def test_customers_are_attached_in_order_order():
    result = attach_customers(orders([30, 10, 20, 40], [2, 1, 2, 3]), CUSTOMERS)
    assert list(result.columns) == ["order_id", "customer_id", "total", "name"]
    assert result["order_id"].tolist() == [30, 10, 20, 40]
    assert result["name"].tolist() == ["bo", "ana", "bo", "cy"]


def test_unused_customers_add_no_rows():
    result = attach_customers(orders([1], [1]), CUSTOMERS)
    assert len(result) == 1


def test_duplicate_customer_keys_raise_merge_error():
    dup = pd.concat([CUSTOMERS, CUSTOMERS.iloc[[0]]], ignore_index=True)
    with pytest.raises(pd.errors.MergeError):
        attach_customers(orders([1, 2], [1, 2]), dup)


def test_unmatched_orders_are_reported_sorted():
    with pytest.raises(UnmatchedOrdersError) as info:
        attach_customers(orders([7, 5, 3], [9, 1, 8]), CUSTOMERS)
    assert info.value.order_ids == [3, 7]
