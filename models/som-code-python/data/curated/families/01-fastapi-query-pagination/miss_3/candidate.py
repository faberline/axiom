"""Item service that lists items page by page with total and page counts."""

from collections.abc import Generator

from fastapi import Depends, FastAPI, Query
from pydantic import BaseModel, ConfigDict
from sqlalchemy import create_engine, func, select
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, sessionmaker
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)
session_factory = sessionmaker(autocommit=False, autoflush=False, bind=engine)


class Base(DeclarativeBase):
    """Declarative base shared by every table in this module."""


class Item(Base):
    """A persisted catalog item."""

    __tablename__ = "items"

    id: Mapped[int] = mapped_column(primary_key=True, index=True, autoincrement=True)
    name: Mapped[str] = mapped_column(nullable=False)


Base.metadata.create_all(bind=engine)


def seed_database() -> None:
    """Insert 25 numbered items when the table is empty."""
    db = session_factory()
    try:
        if db.scalar(select(func.count()).select_from(Item)) == 0:
            for i in range(1, 26):
                db.add(Item(id=i, name=f"Item {i}"))
            db.commit()
    finally:
        db.close()


seed_database()


def reset_db() -> None:
    """Recreate every table and seed it again."""
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    seed_database()


def get_db() -> Generator[Session, None, None]:
    """Yield one database session per request and close it afterwards."""
    db = session_factory()
    try:
        yield db
    finally:
        db.close()


class ItemOut(BaseModel):
    """Item representation returned to clients."""

    id: int
    name: str

    model_config = ConfigDict(from_attributes=True)


class PaginatedResponse(BaseModel):
    """One page of items plus the counts a client needs to page further."""

    items: list[ItemOut]
    total: int
    page: int
    size: int
    pages: int


app = FastAPI(title="Item Pagination Service")


@app.get("/items", response_model=PaginatedResponse)
def list_items(
    page: int = Query(1, ge=1),
    size: int = Query(10, ge=1, le=100),
    db: Session = Depends(get_db),
) -> PaginatedResponse:
    """Return the requested page of items ordered by id."""
    total = db.scalar(select(func.count()).select_from(Item)) or 0
    pages = total // size
    offset = (page - 1) * size

    stmt = select(Item).order_by(Item.id.asc()).offset(offset).limit(size)
    items = db.scalars(stmt).all()

    return PaginatedResponse(
        items=[ItemOut.model_validate(item) for item in items],
        total=total,
        page=page,
        size=size,
        pages=pages,
    )
