import pandas as pd
import pytest

from candidate import load_customers

CSV = (
    "customer_id,zip,country,balance\n"
    "007,02134,US,10.5\n"
    "120,,NA,0\n"
    "0042,00501,NULL,3.25\n"
)


def test_codes_keep_leading_zeros():
    frame = load_customers(CSV)
    assert frame["customer_id"].tolist() == ["007", "120", "0042"]
    assert frame["zip"].iloc[0] == "02134"
    assert frame["zip"].iloc[2] == "00501"


def test_na_like_words_are_real_values():
    frame = load_customers(CSV)
    assert frame["country"].tolist() == ["US", "NA", "NULL"]


def test_blank_cells_are_missing():
    frame = load_customers(CSV)
    assert frame["zip"].isna().tolist() == [False, True, False]


def test_numeric_columns_stay_numeric():
    frame = load_customers(CSV)
    assert pd.api.types.is_float_dtype(frame["balance"])
    assert frame["balance"].sum() == pytest.approx(13.75)


def test_missing_required_column_is_rejected():
    with pytest.raises(ValueError, match="missing columns: zip"):
        load_customers("customer_id,country,balance\n1,US,2\n")
