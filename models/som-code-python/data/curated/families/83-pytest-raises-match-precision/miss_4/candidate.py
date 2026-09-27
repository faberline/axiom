"""Validate withdrawals and assert that a call raises a matching exception."""

import re
from collections.abc import Callable


class TransactionError(ValueError):
    """Base class for rejected transactions."""


class InsufficientFundsError(TransactionError):
    """Raised when a withdrawal exceeds the balance."""


class InvalidAccountStatusError(TransactionError):
    """Raised when the account status forbids withdrawals."""


def process_withdrawal(balance: float, amount: float, status: str) -> float:
    """Return the new balance, raising a typed error for invalid requests."""
    if status != "active":
        raise InvalidAccountStatusError(
            f"Account status '{status}' is invalid for withdrawal"
        )
    if amount <= 0:
        raise ValueError(f"Withdrawal amount {amount} must be strictly positive")
    if amount > balance:
        raise InsufficientFundsError(
            f"Insufficient funds: requested {amount:.2f}, balance {balance:.2f}"
        )
    return balance - amount


def verify_exception_match(
    func: Callable[[], None],
    expected_type: type[Exception],
    exact_pattern: str,
) -> bool:
    """Return True when func raises expected_type with a matching message."""
    try:
        func()
    except Exception as exc:
        msg = str(exc)
        if not re.search(exact_pattern, msg):
            raise AssertionError(
                f"Pattern {exact_pattern!r} did not match exception message: {msg!r}"
            ) from exc
        return True
    raise AssertionError(f"Expected exception {expected_type.__name__} was not raised")
