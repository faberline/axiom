"""Stamp articles on insert and bump their revision only on real changes."""

from datetime import UTC, datetime

from sqlalchemy import Connection, DateTime, String, event
from sqlalchemy.orm import (
    DeclarativeBase,
    Mapped,
    Mapper,
    Session,
    mapped_column,
    object_session,
)


class Base(DeclarativeBase):
    """Declarative base for the publishing tables."""


class Article(Base):
    """An article with audit timestamps and a revision counter."""

    __tablename__ = "articles"

    id: Mapped[int] = mapped_column(primary_key=True)
    title: Mapped[str] = mapped_column(String(200))
    body: Mapped[str]
    created_at: Mapped[datetime] = mapped_column(DateTime, nullable=False)
    updated_at: Mapped[datetime] = mapped_column(DateTime, nullable=False)
    revision: Mapped[int] = mapped_column(default=1)


def now() -> datetime:
    """Return the current time as naive UTC, the form stored in the table."""
    return datetime.now(UTC).replace(tzinfo=None)


@event.listens_for(Article, "before_insert")
def _stamp_created(
    _mapper: Mapper[Article], _connection: Connection, target: Article
) -> None:
    stamp = now()
    target.created_at = stamp
    target.updated_at = stamp


@event.listens_for(Article, "before_update")
def _stamp_updated(
    _mapper: Mapper[Article], _connection: Connection, target: Article
) -> None:
    session = object_session(target)
    if session is None:
        return
    target.updated_at = now()
    target.revision += 1


def edit(
    session: Session,
    article: Article,
    *,
    title: str | None = None,
    body: str | None = None,
) -> None:
    """Apply the given changes to an article and commit them."""
    if title is not None:
        if not title.strip():
            raise ValueError("title must not be blank")
        article.title = title
    if body is not None:
        article.body = body
    session.commit()
