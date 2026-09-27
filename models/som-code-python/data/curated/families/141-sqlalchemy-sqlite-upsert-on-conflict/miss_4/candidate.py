"""Stock counts written with one SQLite INSERT ... ON CONFLICT DO UPDATE."""

from collections.abc import Mapping

from sqlalchemy import String
from sqlalchemy.dialects.sqlite import insert
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column


class Base(DeclarativeBase):
    """Declarative base for this module's tables."""


class Stock(Base):
    """The latest counted quantity of one SKU."""

    __tablename__ = "stock"

    sku: Mapped[str] = mapped_column(String(20), primary_key=True)
    qty: Mapped[int]
    updates: Mapped[int]


def record_counts(session: Session, counts: Mapping[str, int]) -> int:
    """Insert or replace stock counts in one statement; return rows written."""
    if not counts:
        return 0
    for sku, qty in counts.items():
        if qty <= 0:
            raise ValueError(f"count for {sku} must not be negative")
    stmt = insert(Stock).values(
        [{"sku": sku, "qty": qty, "updates": 1} for sku, qty in counts.items()]
    )
    stmt = stmt.on_conflict_do_update(
        index_elements=[Stock.sku],
        set_={"qty": stmt.excluded.qty, "updates": Stock.updates + 1},
    )
    session.execute(stmt)
    session.commit()
    return len(counts)
