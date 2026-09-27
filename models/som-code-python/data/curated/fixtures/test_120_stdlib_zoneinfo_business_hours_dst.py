from datetime import UTC, date, datetime

import pytest

from candidate import business_zone, is_open, opening_utc

NY = business_zone("America/New_York")


def utc(*parts):
    return datetime(*parts, tzinfo=UTC)


def test_opening_moves_with_daylight_saving():
    assert opening_utc(date(2026, 3, 6), NY) == utc(2026, 3, 6, 14, 0)
    assert opening_utc(date(2026, 3, 9), NY) == utc(2026, 3, 9, 13, 0)


def test_open_is_judged_in_local_time():
    assert is_open(utc(2026, 3, 9, 13, 30), NY) is True
    assert is_open(utc(2026, 3, 6, 13, 30), NY) is False


def test_closing_time_is_exclusive_and_weekends_are_closed():
    assert is_open(utc(2026, 3, 9, 20, 59), NY) is True
    assert is_open(utc(2026, 3, 9, 21, 0), NY) is False
    assert is_open(utc(2026, 3, 7, 15, 0), NY) is False


def test_bad_inputs_are_rejected():
    with pytest.raises(ValueError, match="timezone-aware"):
        is_open(datetime(2026, 3, 9, 13, 30), NY)
    with pytest.raises(ValueError, match="unknown time zone"):
        business_zone("Mars/Olympus_Mons")
