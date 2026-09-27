"""Let the database refuse overdrafts through a CHECK constraint."""

from sqlalchemy import CheckConstraint, String
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column


class Base(DeclarativeBase):
    """Declarative base for the ledger tables."""


class Account(Base):
    """A customer account whose balance may never drop below zero."""

    __tablename__ = "accounts"
    __table_args__ = (
        CheckConstraint("balance_cents >= 0", name="balance_not_negative"),
    )

    id: Mapped[int] = mapped_column(primary_key=True)
    owner: Mapped[str] = mapped_column(String(50))
    balance_cents: Mapped[int] = mapped_column(nullable=False)


class InsufficientFundsError(Exception):
    """Raised when a withdrawal would overdraw an account."""


def withdraw(session: Session, account_id: int, amount_cents: int) -> int:
    """Withdraw from an account and return the new balance in cents."""
    if amount_cents <= 0:
        raise ValueError("amount must be positive")
    account = session.get(Account, account_id)
    if account is None:
        raise LookupError(f"account {account_id} not found")
    account.balance_cents -= amount_cents
    try:
        session.commit()
    except IntegrityError as exc:
        raise InsufficientFundsError(
            f"account {account_id} cannot go below zero"
        ) from exc
    return account.balance_cents
