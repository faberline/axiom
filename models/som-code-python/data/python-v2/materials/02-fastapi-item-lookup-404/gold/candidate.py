from typing import Generator
from fastapi import Depends, FastAPI, HTTPException, Path, status
from pydantic import BaseModel
from sqlalchemy import Boolean, Column, Integer, String, create_engine, select
from sqlalchemy.orm import DeclarativeBase, Session, sessionmaker
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)


class Base(DeclarativeBase):
    pass


class Item(Base):
    __tablename__ = "items"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    name = Column(String, nullable=False)
    is_active = Column(Boolean, default=True, nullable=False)


Base.metadata.create_all(bind=engine)


def seed_database() -> None:
    db = SessionLocal()
    try:
        if db.scalar(select(Item).where(Item.id == 1)) is None:
            db.add(Item(id=1, name="Active Widget", is_active=True))
            db.add(Item(id=2, name="Archived Widget", is_active=False))
            db.commit()
    finally:
        db.close()


seed_database()


def reset_db() -> None:
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    seed_database()


def get_db() -> Generator[Session, None, None]:
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()


class ItemOut(BaseModel):
    id: int
    name: str
    is_active: bool

    class Config:
        from_attributes = True


app = FastAPI(title="Item Lookup Service")


@app.get("/items/{item_id}", response_model=ItemOut)
def get_item(
    item_id: int = Path(..., ge=1),
    db: Session = Depends(get_db),
):
    stmt = select(Item).where(Item.id == item_id)
    item = db.scalar(stmt)
    if item is None or not item.is_active:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Item not found",
        )
    return item
