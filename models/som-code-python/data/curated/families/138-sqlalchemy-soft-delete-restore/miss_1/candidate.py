"""Notes that are hidden by a deleted_at timestamp instead of being removed."""

from collections.abc import Sequence
from datetime import datetime

from sqlalchemy import DateTime, String, select
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column


class Base(DeclarativeBase):
    """Declarative base for this module's tables."""


class Note(Base):
    """A note; a non-null deleted_at means it is in the trash."""

    __tablename__ = "notes"

    id: Mapped[int] = mapped_column(primary_key=True)
    title: Mapped[str] = mapped_column(String(100))
    deleted_at: Mapped[datetime | None] = mapped_column(
        DateTime(timezone=True), default=None
    )


def active_notes(session: Session) -> Sequence[Note]:
    """Return notes not in the trash, ordered by title."""
    stmt = select(Note).order_by(Note.title)
    return session.scalars(stmt).all()


def _load(session: Session, note_id: int) -> Note:
    note = session.get(Note, note_id)
    if note is None:
        raise LookupError(f"note {note_id} not found")
    return note


def soft_delete(session: Session, note_id: int, *, now: datetime) -> bool:
    """Trash a note; return False if it was already trashed."""
    note = _load(session, note_id)
    if note.deleted_at is not None:
        return False
    note.deleted_at = now
    session.commit()
    return True


def restore(session: Session, note_id: int) -> bool:
    """Take a note out of the trash; return False if it was not trashed."""
    note = _load(session, note_id)
    if note.deleted_at is None:
        return False
    note.deleted_at = None
    session.commit()
    return True
