"""Documents protected against lost updates by a SQLAlchemy version counter."""

from sqlalchemy import String
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column
from sqlalchemy.orm.exc import StaleDataError


class Base(DeclarativeBase):
    """Declarative base for this module's tables."""


class ConflictError(Exception):
    """Raised when another session changed the row first."""


class Document(Base):
    """A document whose version column guards every UPDATE."""

    __tablename__ = "documents"

    id: Mapped[int] = mapped_column(primary_key=True)
    title: Mapped[str] = mapped_column(String(200))
    version: Mapped[int] = mapped_column(nullable=False)

    __mapper_args__ = {"version_id_col": version}  # noqa: RUF012


def create_document(session: Session, title: str) -> Document:
    """Insert a document and commit it."""
    document = Document(title=title)
    session.add(document)
    session.commit()
    return document


def rename(document: Document, session: Session, title: str) -> None:
    """Change the title, turning a lost-update race into ConflictError."""
    if not title:
        raise ValueError("title must not be blank")
    document_id = document.id
    document.title = title
    try:
        session.commit()
    except StaleDataError as exc:
        session.rollback()
        raise ConflictError(
            f"document {document_id} was changed by someone else"
        ) from exc
