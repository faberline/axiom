import threading
from typing import Optional


class Account:
    def __init__(self, account_id: int, balance: int):
        self.account_id = account_id
        self.balance = balance
        self.lock = threading.Lock()


class SafeTransferManager:
    def transfer(self, source: Account, target: Account, amount: int) -> bool:
        if amount <= 0:
            raise ValueError("Transfer amount must be positive")

        if source.account_id == target.account_id:
            with source.lock:
                if source.balance < amount:
                    return False
                return True

        first, second = (source, target) if source.account_id < target.account_id else (source, target)
        with first.lock:
            with second.lock:
                if source.balance < amount:
                    return False
                source.balance -= amount
                try:
                    target.balance += amount
                except Exception:
                    source.balance += amount
                    raise
                return True
