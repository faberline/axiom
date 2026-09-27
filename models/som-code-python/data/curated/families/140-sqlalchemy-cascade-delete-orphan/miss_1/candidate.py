"""Orders whose lines live and die with them through cascade rules."""

from collections.abc import Sequence

from sqlalchemy import ForeignKey, String
from sqlalchemy.orm import (
    DeclarativeBase,
    Mapped,
    Session,
    mapped_column,
    relationship,
)


class Base(DeclarativeBase):
    """Declarative base for this module's tables."""


class Order(Base):
    """An order owning its lines."""

    __tablename__ = "orders"

    id: Mapped[int] = mapped_column(primary_key=True)
    customer: Mapped[str] = mapped_column(String(50))
    lines: Mapped[list["Line"]] = relationship(
        back_populates="order", cascade="all", order_by="Line.id"
    )


class Line(Base):
    """One product line; it cannot exist without its order."""

    __tablename__ = "lines"

    id: Mapped[int] = mapped_column(primary_key=True)
    order_id: Mapped[int] = mapped_column(ForeignKey("orders.id"), nullable=False)
    sku: Mapped[str] = mapped_column(String(20))
    qty: Mapped[int]
    order: Mapped[Order] = relationship(back_populates="lines")


def _load(session: Session, order_id: int) -> Order:
    order = session.get(Order, order_id)
    if order is None:
        raise LookupError(f"order {order_id} not found")
    return order


def replace_lines(
    session: Session, order_id: int, items: Sequence[tuple[str, int]]
) -> Order:
    """Swap every line of an order for new ones and commit."""
    order = _load(session, order_id)
    for sku, qty in items:
        if qty < 1:
            raise ValueError(f"quantity for {sku} must be at least 1")
    order.lines.clear()
    order.lines.extend(Line(sku=sku, qty=qty) for sku, qty in items)
    session.commit()
    return order


def delete_order(session: Session, order_id: int) -> None:
    """Delete an order together with its lines and commit."""
    session.delete(_load(session, order_id))
    session.commit()
