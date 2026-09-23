"""Item lookup service that hides archived items behind a 404."""

from collections.abc import Generator

from fastapi import Depends, FastAPI, HTTPException, Path, status
from pydantic import BaseModel, ConfigDict
from sqlalchemy import create_engine, select
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
    """A catalog item that can be archived."""

    __tablename__ = "items"

    id: Mapped[int] = mapped_column(primary_key=True, index=True, autoincrement=True)
    name: Mapped[str] = mapped_column(nullable=False)
    is_active: Mapped[bool] = mapped_column(default=True, nullable=False)


Base.metadata.create_all(bind=engine)


def seed_database() -> None:
    """Insert one active and one archived item when the table is empty."""
    db = session_factory()
    try:
        if db.scalar(select(Item).where(Item.id == 1)) is None:
            db.add(Item(id=1, name="Active Widget", is_active=True))
            db.add(Item(id=2, name="Archived Widget", is_active=False))
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
    is_active: bool

    model_config = ConfigDict(from_attributes=True)


app = FastAPI(title="Item Lookup Service")


@app.get("/items/{item_id}", response_model=ItemOut)
def get_item(
    item_id: int = Path(..., ge=1),
    db: Session = Depends(get_db),
) -> Item:
    """Return one active item by id, or 404 when it is missing or archived."""
    stmt = select(Item).where(Item.id == item_id)
    item = db.scalar(stmt)
    if item is None or not item.is_active:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Item not found",
        )
    return item
