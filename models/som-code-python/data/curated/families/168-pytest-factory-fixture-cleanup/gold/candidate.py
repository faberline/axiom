"""A factory fixture that creates accounts on demand and deletes them afterwards."""

import itertools
from collections.abc import Callable, Iterator

import pytest


class AccountStore:
    """In-memory account store that refuses blank and duplicate names."""

    def __init__(self) -> None:
        self.accounts: dict[int, str] = {}
        self.deleted: list[int] = []
        self._next_id = 1

    def create(self, name: str) -> int:
        """Store a new account and return its id."""
        if not name.strip():
            raise ValueError("name must not be blank")
        if name in self.accounts.values():
            raise ValueError(f"duplicate account: {name}")
        account_id = self._next_id
        self._next_id += 1
        self.accounts[account_id] = name
        return account_id

    def delete(self, account_id: int) -> None:
        """Remove an account, recording the deletion."""
        if account_id not in self.accounts:
            raise LookupError(f"no account {account_id}")
        del self.accounts[account_id]
        self.deleted.append(account_id)


@pytest.fixture(name="store")
def store_fixture() -> AccountStore:
    """A fresh, empty account store."""
    return AccountStore()


@pytest.fixture
def make_account(store: AccountStore) -> Iterator[Callable[..., int]]:
    """Yield a factory for accounts and delete every one it made, newest first."""
    created: list[int] = []
    counter = itertools.count(1)

    def factory(name: str | None = None) -> int:
        account_id = store.create(name if name is not None else f"user-{next(counter)}")
        created.append(account_id)
        return account_id

    yield factory
    for account_id in reversed(created):
        if account_id in store.accounts:
            store.delete(account_id)
