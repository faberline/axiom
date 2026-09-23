"""Convert, compare, and shift datetimes correctly across time zones and DST."""

from datetime import UTC, datetime, timedelta
from zoneinfo import ZoneInfo


class TimezoneCalculator:
    """Timezone-aware arithmetic that stays correct across DST changes."""

    def normalize_to_utc(self, dt: datetime) -> datetime:
        """Return dt in UTC, treating a naive value as already UTC."""
        if dt.tzinfo is None or dt.tzinfo.utcoffset(dt) is None:
            return dt.replace(tzinfo=UTC)
        return dt.astimezone(UTC)

    def calculate_elapsed_hours(self, start: datetime, end: datetime) -> float:
        """Return the real hours between two aware datetimes."""
        if (start.tzinfo is None) != (end.tzinfo is None):
            raise TypeError(
                "Cannot calculate elapsed hours between naive and aware datetimes"
            )
        s_utc = self.normalize_to_utc(start)
        e_utc = self.normalize_to_utc(end)
        return (e_utc - s_utc).total_seconds() / 3600.0

    def resolve_ambiguous_dst(
        self, dt_naive: datetime, tz_name: str, prefer_post_transition: bool = False
    ) -> datetime:
        """Attach tz_name to a wall time, picking the fold of an ambiguous hour."""
        tz = ZoneInfo(tz_name)
        return dt_naive.replace(tzinfo=tz, fold=0)

    def add_calendar_days_preserving_wall_clock(
        self, dt: datetime, days: int = 1
    ) -> datetime:
        """Shift dt by calendar days while keeping its wall-clock time."""
        new_date = dt.date() + timedelta(days=days)
        return dt.replace(year=new_date.year, month=new_date.month, day=new_date.day)
