"""Item service: create and read items persisted in SQLite through SQLAlchemy."""

from collections.abc import Generator

from fastapi import Depends, FastAPI, HTTPException, status
from pydantic import BaseModel, ConfigDict, Field
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
    """A persisted catalog item."""

    __tablename__ = "items"

    id: Mapped[int] = mapped_column(primary_key=True, index=True, autoincrement=True)
    name: Mapped[str] = mapped_column(nullable=False)
    price: Mapped[float] = mapped_column(nullable=False)
    description: Mapped[str | None] = mapped_column(nullable=True)


Base.metadata.create_all(bind=engine)


def reset_db() -> None:
    """Drop and recreate every table."""
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)


def get_db() -> Generator[Session, None, None]:
    """Yield one database session per request and close it afterwards."""
    db = session_factory()
    try:
        yield db
    finally:
        db.close()


class ItemCreate(BaseModel):
    """Payload accepted when creating an item."""

    name: str = Field(..., min_length=1)
    price: float = Field(..., ge=1.0)
    description: str | None = None


class ItemOut(BaseModel):
    """Item representation returned to clients."""

    model_config = ConfigDict(from_attributes=True)

    id: int
    name: str
    price: float
    description: str | None = None


app = FastAPI(title="Item Service")


@app.post("/items", response_model=ItemOut, status_code=status.HTTP_201_CREATED)
def create_item(payload: ItemCreate, db: Session = Depends(get_db)) -> Item:
    """Persist a new item and return it."""
    item = Item(
        name=payload.name,
        price=payload.price,
        description=payload.description,
    )
    db.add(item)
    db.commit()
    db.refresh(item)
    return item


@app.get("/items/{item_id}", response_model=ItemOut)
def get_item(item_id: int, db: Session = Depends(get_db)) -> Item:
    """Return one item by id, or 404 when it does not exist."""
    stmt = select(Item).where(Item.id == item_id)
    item = db.scalar(stmt)
    if item is None:
        raise HTTPException(status_code=404, detail="Item not found")
    return item
