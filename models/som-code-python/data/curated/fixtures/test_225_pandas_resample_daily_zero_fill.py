import pandas as pd
import pytest

from candidate import daily_signups

EVENTS = pd.DataFrame(
    {
        "created_at": [
            "2024-01-03T12:00:00Z",
            "2024-01-01T08:00:00Z",
            "2024-01-01T23:30:00-02:00",
            "2024-01-03T01:00:00Z",
            "2024-01-03T02:00:00Z",
            "2023-12-31T10:00:00Z",
        ]
    }
)


def test_every_day_in_range_is_present_with_zero_gaps():
    result = daily_signups(EVENTS, "2024-01-01", "2024-01-05")
    assert result.tolist() == [1, 1, 3, 0, 0]
    assert result.dtype == "int64"


def test_index_is_utc_days_named_day():
    result = daily_signups(EVENTS, "2024-01-01", "2024-01-03")
    assert result.index.name == "day"
    assert str(result.index.tz) == "UTC"
    assert result.index.strftime("%m-%d").tolist() == ["01-01", "01-02", "01-03"]
    assert result.name == "signups"


def test_offsets_are_bucketed_by_utc_day():
    result = daily_signups(EVENTS, "2024-01-02", "2024-01-02")
    assert result.tolist() == [1]


def test_reversed_range_is_rejected():
    with pytest.raises(ValueError, match="start must not be after end"):
        daily_signups(EVENTS, "2024-01-05", "2024-01-01")
