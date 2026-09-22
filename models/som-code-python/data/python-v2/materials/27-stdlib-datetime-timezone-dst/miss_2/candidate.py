from datetime import datetime, timezone, timedelta
from zoneinfo import ZoneInfo


class TimezoneCalculator:
    def normalize_to_utc(self, dt: datetime) -> datetime:
        if dt.tzinfo is None or dt.tzinfo.utcoffset(dt) is None:
            return dt.replace(tzinfo=timezone.utc)
        return dt.astimezone(timezone.utc)

    def calculate_elapsed_hours(self, start: datetime, end: datetime) -> float:
        s_utc = self.normalize_to_utc(start)
        e_utc = self.normalize_to_utc(end)
        return (e_utc - s_utc).total_seconds() / 3600.0

    def resolve_ambiguous_dst(self, dt_naive: datetime, tz_name: str, prefer_post_transition: bool = False) -> datetime:
        tz = ZoneInfo(tz_name)
        fold = 1 if prefer_post_transition else 0
        return dt_naive.replace(tzinfo=tz, fold=fold)

    def add_calendar_days_preserving_wall_clock(self, dt: datetime, days: int = 1) -> datetime:
        new_date = dt.date() + timedelta(days=days)
        return dt.replace(year=new_date.year, month=new_date.month, day=new_date.day)
