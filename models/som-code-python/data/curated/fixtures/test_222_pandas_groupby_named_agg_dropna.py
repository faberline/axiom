import pandas as pd

from candidate import region_summary

SALES = pd.DataFrame(
    {
        "order_id": [1, 1, 2, 3, 4, 5, 6, 7],
        "region": ["a", "a", "a", "b", "b", "b", "c", None],
        "amount": [10.0, 20.0, 60.0, 10.0, 10.0, 11.0, 31.0, 5.0],
    }
)


def test_columns_and_revenue_order():
    result = region_summary(SALES)
    assert list(result.columns) == ["region", "orders", "revenue", "avg_ticket"]
    assert result["region"].tolist() == ["a", "b", "c", "unknown"]
    assert result["revenue"].tolist() == [90.0, 31.0, 31.0, 5.0]


def test_orders_count_distinct_order_ids():
    result = region_summary(SALES)
    assert result["orders"].tolist() == [2, 3, 1, 1]


def test_average_ticket_is_rounded_mean():
    result = region_summary(SALES)
    assert result["avg_ticket"].tolist() == [30.0, 10.33, 31.0, 5.0]


def test_missing_region_is_kept_as_unknown():
    result = region_summary(SALES.iloc[[7]])
    assert result["region"].tolist() == ["unknown"]
    assert result["orders"].tolist() == [1]


def test_input_is_not_modified():
    before = SALES.copy()
    region_summary(SALES)
    pd.testing.assert_frame_equal(SALES, before)
