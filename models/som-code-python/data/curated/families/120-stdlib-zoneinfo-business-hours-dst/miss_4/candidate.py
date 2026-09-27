"""Decide whether an instant falls inside a shop's local business hours."""

from datetime import UTC, date, datetime, time
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

OPEN = time(9)
CLOSE = time(17)


def business_zone(name: str) -> ZoneInfo:
    """Load an IANA time zone, turning lookup failures into ValueError."""
    try:
        return ZoneInfo(name)
    except (ZoneInfoNotFoundError, ValueError) as exc:
        raise ValueError(f"unknown time zone: {name}") from exc


def is_open(instant: datetime, zone: ZoneInfo) -> bool:
    """Return whether an aware instant is within weekday hours in zone."""
    local = instant.astimezone(zone)
    return local.weekday() < 5 and OPEN <= local.time() < CLOSE


def opening_utc(day: date, zone: ZoneInfo) -> datetime:
    """Return the UTC instant at which the shop opens on a local calendar day."""
    return datetime.combine(day, OPEN, tzinfo=zone).astimezone(UTC)
