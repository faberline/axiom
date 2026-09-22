import re
from typing import Callable, Type

class TransactionError(ValueError):
    pass

class InsufficientFundsError(TransactionError):
    pass

class InvalidAccountStatusError(TransactionError):
    pass

def process_withdrawal(balance: float, amount: float, status: str) -> float:
    if status != "active":
        raise InvalidAccountStatusError(f"Account status '{status}' is invalid for withdrawal")
    if amount <= 0:
        raise ValueError(f"Withdrawal amount {amount} must be strictly positive")
    if amount > balance:
        raise InsufficientFundsError(f"Insufficient funds: requested {amount:.2f}, balance {balance:.2f}")
    return balance - amount

def verify_exception_match(
    func: Callable[[], None],
    expected_type: Type[Exception],
    exact_pattern: str,
) -> bool:
    try:
        func()
    except expected_type as exc:
        msg = str(exc)
        if not exact_pattern:
            raise AssertionError(f"Pattern {exact_pattern!r} did not match exception message: {msg!r}")
        return True
    except Exception as exc:
        raise AssertionError(f"Expected {expected_type.__name__}, got {type(exc).__name__}: {exc}")
    raise AssertionError(f"Expected exception {expected_type.__name__} was not raised")
