"""Per-region sales totals filtered on the aggregate with HAVING."""

from sqlalchemy import String, func, select
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column


class Base(DeclarativeBase):
    """Declarative base for this module's tables."""


class Sale(Base):
    """One sale in integer cents."""

    __tablename__ = "sales"

    id: Mapped[int] = mapped_column(primary_key=True)
    region: Mapped[str] = mapped_column(String(30), index=True)
    amount_cents: Mapped[int]


def top_regions(
    session: Session, *, min_total: int, limit: int = 3
) -> list[tuple[str, int, int]]:
    """Return (region, total, count) for regions totalling at least min_total."""
    if limit < 1:
        raise ValueError("limit must be at least 1")
    total = func.sum(Sale.amount_cents).label("total")
    stmt = (
        select(Sale.region, total, func.count(Sale.id))
        .group_by(Sale.region)
        .having(total >= min_total)
        .order_by(total.desc(), Sale.region)
        .limit(limit)
    )
    return [
        (region, int(amount), count) for region, amount, count in session.execute(stmt)
    ]
