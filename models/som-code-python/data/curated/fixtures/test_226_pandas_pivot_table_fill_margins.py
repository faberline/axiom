import pandas as pd

from candidate import revenue_matrix

SALES = pd.DataFrame(
    {
        "date": [
            "2024-01-05",
            "2024-01-20",
            "2024-03-02",
            "2024-02-11",
            "2024-01-09",
            "2024-02-28",
        ],
        "product": ["pen", "pen", "pen", "cup", "ink", "ink"],
        "amount": [10, 5, 7, 20, 3, 4],
    }
)


def test_rows_are_products_and_columns_are_months():
    matrix = revenue_matrix(SALES)
    assert matrix.index.tolist() == ["cup", "ink", "pen", "total"]
    assert matrix.columns.tolist() == ["2024-01", "2024-02", "2024-03", "total"]


def test_duplicate_lines_are_summed():
    matrix = revenue_matrix(SALES)
    assert matrix.loc["pen", "2024-01"] == 15


def test_missing_combinations_are_zero():
    matrix = revenue_matrix(SALES)
    assert matrix.loc["cup"].tolist() == [0, 20, 0, 20]
    assert not matrix.isna().any().any()


def test_margins_hold_row_and_column_totals():
    matrix = revenue_matrix(SALES)
    assert matrix["total"].tolist() == [20, 7, 22, 49]
    assert matrix.loc["total"].tolist() == [18, 24, 7, 49]
