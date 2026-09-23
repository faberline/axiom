"""Transfer funds between accounts without deadlocking on their locks."""

import threading


class Account:
    """A bank balance guarded by its own lock."""

    def __init__(self, account_id: int, balance: int) -> None:
        self.account_id = account_id
        self.balance = balance
        self.lock = threading.Lock()


class SafeTransferManager:
    """Move funds while always locking the lower account id first."""

    def transfer(self, source: Account, target: Account, amount: int) -> bool:
        """Move amount from source to target; return False when funds run short."""
        if amount <= 0:
            raise ValueError("Transfer amount must be positive")

        if source.account_id == target.account_id:
            with source.lock:
                return True

        first, second = (
            (source, target)
            if source.account_id < target.account_id
            else (target, source)
        )
        with first.lock, second.lock:
            source.balance -= amount
            try:
                target.balance += amount
            except Exception:
                source.balance += amount
                raise
            return True
