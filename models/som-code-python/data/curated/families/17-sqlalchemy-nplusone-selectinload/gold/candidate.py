"""Order listing that loads line items with one extra SELECT via selectinload."""

from __future__ import annotations

from collections.abc import Generator, Sequence
from typing import Any, ClassVar

from fastapi import Depends, FastAPI
from pydantic import BaseModel, ConfigDict
from sqlalchemy import (
    ForeignKey,
    create_engine,
    event,
    select,
)
from sqlalchemy.orm import (
    DeclarativeBase,
    Mapped,
    Session,
    mapped_column,
    relationship,
    selectinload,
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


class Order(Base):
    """An order and its line items."""

    __tablename__ = "orders"

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    order_number: Mapped[str] = mapped_column(nullable=False)
    status: Mapped[str] = mapped_column(default="completed", nullable=False)

    items: Mapped[list[OrderItem]] = relationship(
        "OrderItem", back_populates="order", lazy="select"
    )


class OrderItem(Base):
    """One product line of an order."""

    __tablename__ = "order_items"

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    order_id: Mapped[int] = mapped_column(ForeignKey("orders.id"), nullable=False)
    product_name: Mapped[str] = mapped_column(nullable=False)
    quantity: Mapped[int] = mapped_column(nullable=False)
    price: Mapped[float] = mapped_column(nullable=False)

    order: Mapped[Order] = relationship("Order", back_populates="items")


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
    """Recreate the tables and seed five orders of two items each."""
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    db = session_factory()
    for i in range(1, 6):
        order = Order(order_number=f"ORD-{i:03d}", status="completed")
        db.add(order)
        db.flush()
        item1 = OrderItem(
            order_id=order.id, product_name=f"Item {i}-A", quantity=1, price=10.0
        )
        item2 = OrderItem(
            order_id=order.id, product_name=f"Item {i}-B", quantity=2, price=20.0
        )
        db.add_all([item1, item2])
    db.commit()
    db.close()
    QueryCounter.reset()


reset_db()


class OrderItemResponse(BaseModel):
    """Order line representation returned to clients."""

    id: int
    product_name: str
    quantity: int
    price: float

    model_config = ConfigDict(from_attributes=True)


class OrderResponse(BaseModel):
    """Order representation returned to clients."""

    id: int
    order_number: str
    status: str
    items: list[OrderItemResponse]

    model_config = ConfigDict(from_attributes=True)


app = FastAPI()


@app.get("/query-metrics")
def get_query_metrics() -> dict[str, int]:
    """Return how many SELECT statements have run since the last reset."""
    return {"count": QueryCounter.count}


@app.get("/orders", response_model=list[OrderResponse])
def get_orders(
    include_details: bool = True, db: Session = Depends(get_db)
) -> Sequence[Order]:
    """Return every order, eager-loading items unless details are declined."""
    stmt = select(Order)
    if include_details:
        stmt = stmt.options(selectinload(Order.items))
    orders = db.scalars(stmt).all()
    return orders
