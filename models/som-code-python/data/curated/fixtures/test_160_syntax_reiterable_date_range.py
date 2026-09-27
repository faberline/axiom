from datetime import date, datetime

import pytest

from candidate import DateRange

JAN = [date(2026, 1, d) for d in (1, 4, 7)]


def test_iterates_half_open_with_step():
    assert list(DateRange(date(2026, 1, 1), date(2026, 1, 8), 3)) == JAN
    assert list(DateRange(date(2026, 1, 1), date(2026, 1, 7), 3)) == JAN[:2]


def test_can_be_iterated_repeatedly():
    week = DateRange(date(2026, 1, 1), date(2026, 1, 8), 3)
    assert list(week) == JAN
    assert list(week) == JAN
    assert [(a, b) for a in week for b in week][-1] == (JAN[-1], JAN[-1])


def test_len_rounds_up_partial_steps():
    assert len(DateRange(date(2026, 1, 1), date(2026, 1, 8), 3)) == 3
    assert len(DateRange(date(2026, 1, 1), date(2026, 1, 7), 3)) == 2
    assert len(DateRange(date(2026, 1, 1), date(2026, 1, 1))) == 0


def test_len_matches_iteration():
    r = DateRange(date(2026, 2, 1), date(2026, 3, 1), 5)
    assert len(r) == len(list(r)) == 6


@pytest.mark.parametrize(
    ("item", "expected"),
    [
        (date(2026, 1, 4), True),
        (date(2026, 1, 5), False),
        (date(2026, 1, 10), False),
        (date(2025, 12, 29), False),
        (datetime(2026, 1, 4), False),
        ("2026-01-04", False),
    ],
)
def test_membership(item, expected):
    assert (item in DateRange(date(2026, 1, 1), date(2026, 1, 10), 3)) is expected


def test_invalid_ranges_are_rejected():
    with pytest.raises(ValueError, match="step_days must be at least 1"):
        DateRange(date(2026, 1, 1), date(2026, 1, 2), 0)
    with pytest.raises(ValueError, match="stop must not be before start"):
        DateRange(date(2026, 1, 2), date(2026, 1, 1))
