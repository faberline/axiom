from datetime import datetime, timezone
from zoneinfo import ZoneInfo
import pytest
from candidate import TimezoneCalculator


def test_normalize_to_utc_aware():
    calc = TimezoneCalculator()
    ny = ZoneInfo("America/New_York")
    dt = datetime(2024, 1, 1, 12, 0, 0, tzinfo=ny)
    res = calc.normalize_to_utc(dt)
    assert res.tzinfo == timezone.utc
    assert res.hour == 17
    assert res.day == 1


def test_calculate_elapsed_hours_requires_timezone_aware():
    calc = TimezoneCalculator()
    dt_naive1 = datetime(2024, 1, 1, 10, 0, 0)
    dt_naive2 = datetime(2024, 1, 1, 12, 0, 0)
    with pytest.raises(ValueError):
        calc.calculate_elapsed_hours(dt_naive1, dt_naive2)


def test_resolve_ambiguous_dst_fold():
    calc = TimezoneCalculator()
    naive = datetime(2024, 11, 3, 1, 30, 0)
    dt_pre = calc.resolve_ambiguous_dst(naive, "America/New_York", prefer_post_transition=False)
    dt_post = calc.resolve_ambiguous_dst(naive, "America/New_York", prefer_post_transition=True)
    assert dt_pre.fold == 0
    assert dt_post.fold == 1
    delta_seconds = (dt_post.astimezone(timezone.utc) - dt_pre.astimezone(timezone.utc)).total_seconds()
    assert delta_seconds == 3600.0, f"Expected 1 hour (3600s) difference, got {delta_seconds}"


def test_add_calendar_days_preserves_wall_clock():
    calc = TimezoneCalculator()
    ny = ZoneInfo("America/New_York")
    dt_before = datetime(2024, 3, 9, 3, 30, 0, tzinfo=ny)
    dt_after = calc.add_calendar_days_preserving_wall_clock(dt_before, days=1)
    assert dt_after.day == 10
    assert dt_after.hour == 3
    assert dt_after.minute == 30


def test_calculate_elapsed_hours_multi_day():
    calc = TimezoneCalculator()
    s = datetime(2024, 1, 1, 10, 0, 0, tzinfo=timezone.utc)
    e = datetime(2024, 1, 3, 16, 0, 0, tzinfo=timezone.utc)
    hrs = calc.calculate_elapsed_hours(s, e)
    assert hrs == 54.0, f"Expected 54.0 hours, got {hrs}"
