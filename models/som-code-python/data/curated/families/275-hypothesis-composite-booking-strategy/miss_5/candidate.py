"""Hypothesis composite strategies that only build valid hotel bookings."""

from __future__ import annotations

import datetime as dt
from dataclasses import dataclass

from hypothesis import strategies as st

EPOCH = dt.date(2024, 1, 1)
MAX_NIGHTS = 30


@dataclass(frozen=True)
class Booking:
    """A stay from check_in to check_out, check_out exclusive."""

    check_in: dt.date
    check_out: dt.date

    def nights(self) -> int:
        """Return the number of nights in the stay."""
        return (self.check_out - self.check_in).days

    def overlaps(self, other: Booking) -> bool:
        """Return whether the stays share a night; back-to-back is fine."""
        return self.check_in < other.check_out and other.check_in < self.check_out


def make_booking(check_in: dt.date, check_out: dt.date) -> Booking:
    """Build a booking or raise ValueError outside 1..MAX_NIGHTS nights."""
    nights = (check_out - check_in).days
    if not 1 <= nights < MAX_NIGHTS:
        raise ValueError(f"a stay must last 1 to {MAX_NIGHTS} nights")
    return Booking(check_in, check_out)


@st.composite
def bookings(draw: st.DrawFn, max_nights: int = MAX_NIGHTS) -> Booking:
    """Draw a check-in within a year of EPOCH and a valid stay length."""
    check_in = draw(st.dates(min_value=EPOCH, max_value=EPOCH + dt.timedelta(days=365)))
    nights = draw(st.integers(min_value=1, max_value=max_nights))
    return make_booking(check_in, check_in + dt.timedelta(days=nights))
