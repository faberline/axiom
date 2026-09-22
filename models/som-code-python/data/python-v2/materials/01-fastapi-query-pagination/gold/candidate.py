from typing import Generator
from fastapi import Depends, FastAPI, HTTPException, Query, status
from pydantic import BaseModel
from sqlalchemy import Column, Integer, String, create_engine, func, select
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


Base.metadata.create_all(bind=engine)


def seed_database() -> None:
    db = SessionLocal()
    try:
        if db.scalar(select(func.count()).select_from(Item)) == 0:
            for i in range(1, 26):
                db.add(Item(id=i, name=f"Item {i}"))
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

    class Config:
        from_attributes = True


class PaginatedResponse(BaseModel):
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
):
    total = db.scalar(select(func.count()).select_from(Item)) or 0
    pages = (total + size - 1) // size if total > 0 else 0
    offset = (page - 1) * size

    stmt = (
        select(Item)
        .order_by(Item.id.asc())
        .offset(offset)
        .limit(size)
    )
    items = db.scalars(stmt).all()

    return PaginatedResponse(
        items=list(items),
        total=total,
        page=page,
        size=size,
        pages=pages,
    )
