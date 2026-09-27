"""Products whose sale price is one expression usable in Python and in SQL."""

from sqlalchemy import String, select
from sqlalchemy.ext.hybrid import hybrid_property
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, validates


class Base(DeclarativeBase):
    """Declarative base for this module's tables."""


class Product(Base):
    """A product with a list price and a percentage discount."""

    __tablename__ = "products"

    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column(String(50))
    price_cents: Mapped[int]
    discount_pct: Mapped[int] = mapped_column(default=0)

    @validates("discount_pct")
    def _check_discount(self, _key: str, value: int) -> int:
        if not 0 <= value <= 90:
            raise ValueError("discount must be between 0 and 90")
        return value

    @hybrid_property
    def sale_price_cents(self) -> int:
        """Discounted price in whole cents, rounded down."""
        return self.price_cents * (100 - self.discount_pct) / 100


def on_sale_under(session: Session, max_cents: int) -> list[str]:
    """Names of discounted products costing at most max_cents, cheapest first."""
    stmt = (
        select(Product.name)
        .where(Product.discount_pct > 0)
        .where(Product.sale_price_cents <= max_cents)
        .order_by(Product.sale_price_cents, Product.name)
    )
    return list(session.scalars(stmt))
