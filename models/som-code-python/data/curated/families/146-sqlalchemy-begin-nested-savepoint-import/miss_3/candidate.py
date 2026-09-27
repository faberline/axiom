"""Contact import that skips bad rows using one savepoint per row."""

from collections.abc import Iterable

from sqlalchemy import String
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column


class Base(DeclarativeBase):
    """Declarative base for this module's tables."""


class Contact(Base):
    """A contact identified by a unique lower-case email."""

    __tablename__ = "contacts"

    id: Mapped[int] = mapped_column(primary_key=True)
    email: Mapped[str] = mapped_column(String(254), unique=True)
    name: Mapped[str] = mapped_column(String(100))


def import_contacts(session: Session, rows: Iterable[tuple[str, str]]) -> list[str]:
    """Insert rows, each in its own savepoint; return the emails rejected."""
    rejected: list[str] = []
    for email, name in rows:
        if "@" not in email:
            rejected.append(email)
            continue
        try:
            with session.begin_nested():
                session.add(Contact(email=email.lower(), name=name))
        except IntegrityError:
            rejected.append(email)
    session.flush()
    return rejected
