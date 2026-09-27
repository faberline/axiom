"""Item service whose PATCH route changes only the fields a client sends."""

from collections.abc import Generator

from fastapi import Depends, FastAPI, HTTPException
from pydantic import BaseModel, ConfigDict
from sqlalchemy import create_engine
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

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    name: Mapped[str] = mapped_column(nullable=False)
    description: Mapped[str | None] = mapped_column(nullable=True)
    price: Mapped[float] = mapped_column(nullable=False)


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

    name: str
    description: str | None = None
    price: float


class ItemUpdate(BaseModel):
    """Partial payload: every field is optional and only sent fields apply."""

    name: str | None = None
    description: str | None = None
    price: float | None = None


class ItemResponse(BaseModel):
    """Item representation returned to clients."""

    id: int
    name: str
    description: str | None
    price: float

    model_config = ConfigDict(from_attributes=True)


app = FastAPI()


@app.post("/items", status_code=201, response_model=ItemResponse)
def create_item(payload: ItemCreate, db: Session = Depends(get_db)) -> Item:
    """Persist a new item and return it."""
    item = Item(
        name=payload.name,
        description=payload.description,
        price=payload.price,
    )
    db.add(item)
    db.commit()
    db.refresh(item)
    return item


@app.get("/items/{item_id}", response_model=ItemResponse)
def get_item(item_id: int, db: Session = Depends(get_db)) -> Item:
    """Return one item by id, or 404 when it does not exist."""
    item = db.get(Item, item_id)
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")
    return item


@app.patch("/items/{item_id}", response_model=ItemResponse)
def patch_item(
    item_id: int, payload: ItemUpdate, db: Session = Depends(get_db)
) -> Item:
    """Apply the fields the client sent to an existing item."""
    item = db.get(Item, item_id)
    if not item:
        raise HTTPException(status_code=404, detail="Item not found")

    update_data = payload.model_dump(exclude_unset=True)
    for key, value in update_data.items():
        setattr(item, key, value)

    return item
