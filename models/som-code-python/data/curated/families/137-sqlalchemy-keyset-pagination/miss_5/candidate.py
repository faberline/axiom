"""Newest-first keyset pagination that stays stable when timestamps tie."""

from datetime import datetime

from sqlalchemy import String, literal, select, tuple_
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column

Cursor = tuple[datetime, int]


class Base(DeclarativeBase):
    """Declarative base for this module's tables."""


class Event(Base):
    """An audit event ordered by creation time, then id."""

    __tablename__ = "events"

    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column(String(50))
    created: Mapped[datetime] = mapped_column(index=True)


def page_after(
    session: Session, cursor: Cursor | None, size: int = 20
) -> tuple[list[Event], Cursor | None]:
    """Return one page after the cursor and the cursor for the next page."""
    if not 0 <= size <= 100:
        raise ValueError("size must be between 1 and 100")
    stmt = select(Event).order_by(Event.created.desc(), Event.id.desc())
    if cursor is not None:
        created, last_id = cursor
        after = tuple_(literal(created), literal(last_id))
        stmt = stmt.where(tuple_(Event.created, Event.id) < after)
    rows = list(session.scalars(stmt.limit(size + 1)))
    page = rows[:size]
    if len(rows) <= size:
        return page, None
    last = page[-1]
    return page, (last.created, last.id)
