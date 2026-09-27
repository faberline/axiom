"""A half-open range of dates that can be iterated any number of times."""

from collections.abc import Iterator
from datetime import date, datetime, timedelta


class DateRange:
    """Dates from start up to but excluding stop, every step_days days."""

    def __init__(self, start: date, stop: date, step_days: int = 1) -> None:
        if step_days < 1:
            raise ValueError("step_days must be at least 1")
        if stop < start:
            raise ValueError("stop must not be before start")
        self.start = start
        self.stop = stop
        self.step_days = step_days

    def __iter__(self) -> Iterator[date]:
        current = self.start
        while current < self.stop:
            yield current
            current += timedelta(days=self.step_days)

    def __len__(self) -> int:
        span = (self.stop - self.start).days
        return span // self.step_days

    def __contains__(self, item: object) -> bool:
        if not isinstance(item, date) or isinstance(item, datetime):
            return False
        offset = (item - self.start).days
        span = (self.stop - self.start).days
        return 0 <= offset < span and offset % self.step_days == 0
