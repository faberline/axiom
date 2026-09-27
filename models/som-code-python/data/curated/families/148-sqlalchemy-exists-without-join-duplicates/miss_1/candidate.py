"""Answer yes-or-no questions about invoices with EXISTS instead of joins."""

from sqlalchemy import ForeignKey, String, exists, select
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column


class Base(DeclarativeBase):
    """Declarative base for the billing tables."""


class Customer(Base):
    """A billed customer."""

    __tablename__ = "customers"

    id: Mapped[int] = mapped_column(primary_key=True)
    name: Mapped[str] = mapped_column(String(50))


class Invoice(Base):
    """One invoice for a customer, paid or not."""

    __tablename__ = "invoices"

    id: Mapped[int] = mapped_column(primary_key=True)
    customer_id: Mapped[int] = mapped_column(ForeignKey("customers.id"))
    amount_cents: Mapped[int]
    paid: Mapped[bool] = mapped_column(default=False)


def has_unpaid(session: Session, customer_id: int) -> bool:
    """Return whether the customer has any unpaid invoice, loading no rows."""
    unpaid = exists().where(Invoice.customer_id == customer_id, Invoice.paid.is_(False))
    return bool(session.scalar(select(unpaid)))


def customers_owing(session: Session, min_cents: int) -> list[str]:
    """Names of customers with an unpaid invoice of at least min_cents."""
    if min_cents < 0:
        raise ValueError("min_cents must not be negative")
    stmt = (
        select(Customer.name)
        .join(Invoice, Invoice.customer_id == Customer.id)
        .where(Invoice.paid.is_(False), Invoice.amount_cents >= min_cents)
        .order_by(Customer.name)
    )
    return list(session.scalars(stmt))


def customers_without_invoices(session: Session) -> list[str]:
    """Names of customers that have never been invoiced."""
    invoiced = exists().where(Invoice.customer_id == Customer.id)
    stmt = select(Customer.name).where(~invoiced).order_by(Customer.name)
    return list(session.scalars(stmt))
