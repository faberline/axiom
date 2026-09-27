import datetime as dt

import pytest
from hypothesis import find, given, settings

from candidate import EPOCH, Booking, bookings, make_booking

FIXED = settings(max_examples=150, derandomize=True, database=None, deadline=None)


@FIXED
@given(bookings())
def test_strategy_yields_valid_stays_near_epoch(booking):
    assert 1 <= booking.nights() <= 30
    assert EPOCH <= booking.check_in <= EPOCH + dt.timedelta(days=365)


@FIXED
@given(bookings(max_nights=3))
def test_max_nights_argument_narrows_the_strategy(booking):
    assert booking.nights() <= 3


@FIXED
@given(bookings())
def test_back_to_back_stays_do_not_overlap(booking):
    following = Booking(booking.check_out, booking.check_out + dt.timedelta(days=1))
    assert not booking.overlaps(following)
    assert not following.overlaps(booking)
    assert booking.overlaps(booking)


def test_strategy_reaches_both_length_bounds():
    assert find(bookings(), lambda b: b.nights() == 30, settings=FIXED).nights() == 30
    assert find(bookings(), lambda b: b.nights() == 1, settings=FIXED).nights() == 1


def test_make_booking_rejects_bad_lengths():
    day = dt.date(2024, 5, 1)
    assert make_booking(day, day + dt.timedelta(days=30)).nights() == 30
    for nights in (0, -1, 31):
        with pytest.raises(ValueError):
            make_booking(day, day + dt.timedelta(days=nights))
