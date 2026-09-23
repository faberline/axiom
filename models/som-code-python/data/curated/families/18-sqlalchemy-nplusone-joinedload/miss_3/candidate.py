"""Article listing that loads authors in the same SELECT via joinedload."""

from __future__ import annotations

from collections.abc import Generator, Sequence
from typing import Any, ClassVar

from fastapi import Depends, FastAPI
from pydantic import BaseModel, ConfigDict
from sqlalchemy import ForeignKey, create_engine, event, select
from sqlalchemy.orm import (
    DeclarativeBase,
    Mapped,
    Session,
    joinedload,
    mapped_column,
    relationship,
    sessionmaker,
)
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)


class QueryCounter:
    """Count the SELECT statements the engine executes."""

    count: ClassVar[int] = 0
    statements: ClassVar[list[str]] = []

    @classmethod
    def reset(cls) -> None:
        """Start counting from zero."""
        cls.count = 0
        cls.statements = []


@event.listens_for(engine, "before_cursor_execute", named=True)
def before_cursor_execute(**kw: Any) -> None:
    """Record every SELECT the engine sends to the database."""
    statement: str = kw["statement"]
    if statement.strip().upper().startswith("SELECT"):
        QueryCounter.count += 1
        QueryCounter.statements.append(statement)


class Base(DeclarativeBase):
    """Declarative base shared by every table in this module."""


class Author(Base):
    """An article author."""

    __tablename__ = "authors"

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    name: Mapped[str] = mapped_column(nullable=False)
    email: Mapped[str] = mapped_column(nullable=False)


class Article(Base):
    """An article with an optional author."""

    __tablename__ = "articles"

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    title: Mapped[str] = mapped_column(nullable=False)
    content: Mapped[str] = mapped_column(nullable=False)
    author_id: Mapped[int | None] = mapped_column(
        ForeignKey("authors.id"), nullable=True
    )

    author: Mapped[Author | None] = relationship("Author", lazy="select")


Base.metadata.create_all(bind=engine)
session_factory = sessionmaker(autocommit=False, autoflush=False, bind=engine)


def get_db() -> Generator[Session, None, None]:
    """Yield one database session per request and close it afterwards."""
    db = session_factory()
    try:
        yield db
    finally:
        db.close()


def reset_db() -> None:
    """Recreate the tables and seed two authors and four articles."""
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    db = session_factory()
    a1 = Author(name="Alice Smith", email="alice@example.com")
    a2 = Author(name="Bob Jones", email="bob@example.com")
    db.add_all([a1, a2])
    db.flush()

    art1 = Article(
        title="Deep Dive into Async", content="Async content", author_id=a1.id
    )
    art2 = Article(
        title="SQLAlchemy Patterns", content="SQLAlchemy content", author_id=a1.id
    )
    art3 = Article(
        title="Distributed Systems", content="Systems content", author_id=a2.id
    )
    art4 = Article(
        title="Anonymous Editorial", content="Opinion content", author_id=None
    )
    db.add_all([art1, art2, art3, art4])
    db.commit()
    db.close()
    QueryCounter.reset()


reset_db()


class AuthorResponse(BaseModel):
    """Author representation returned to clients."""

    id: int
    name: str
    email: str

    model_config = ConfigDict(from_attributes=True)


class ArticleResponse(BaseModel):
    """Article representation returned to clients."""

    id: int
    title: str
    content: str
    author: AuthorResponse | None = None

    model_config = ConfigDict(from_attributes=True)


app = FastAPI()


@app.get("/query-metrics")
def get_query_metrics() -> dict[str, int]:
    """Return how many SELECT statements have run since the last reset."""
    return {"count": QueryCounter.count}


@app.get("/articles", response_model=list[ArticleResponse])
def get_articles(db: Session = Depends(get_db)) -> Sequence[Article]:
    """Return every article with its author loaded by a join."""
    stmt = select(Article).options(joinedload(Article.author))
    articles = db.scalars(stmt).all()[1:]
    return articles
