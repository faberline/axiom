import pytest
from sqlalchemy import create_engine
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from candidate import Account, Base, InsufficientFundsError, withdraw


@pytest.fixture
def session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    with Session(engine) as s:
        s.add(Account(id=1, owner="alice", balance_cents=1000))
        s.commit()
        yield s
    engine.dispose()


def test_withdrawal_reduces_and_commits_the_balance(session):
    assert withdraw(session, 1, 300) == 700
    session.rollback()
    assert session.get(Account, 1).balance_cents == 700


def test_withdrawing_to_exactly_zero_is_allowed(session):
    assert withdraw(session, 1, 1000) == 0


def test_overdraft_raises_and_keeps_the_balance(session):
    with pytest.raises(InsufficientFundsError, match="account 1 cannot go below zero"):
        withdraw(session, 1, 1001)
    assert session.get(Account, 1).balance_cents == 1000


def test_the_table_itself_rejects_negative_balances(session):
    session.add(Account(id=2, owner="bob", balance_cents=-1))
    with pytest.raises(IntegrityError):
        session.commit()


@pytest.mark.parametrize("amount", [0, -5])
def test_non_positive_amounts_are_rejected(session, amount):
    with pytest.raises(ValueError, match="amount must be positive"):
        withdraw(session, 1, amount)


def test_unknown_account_is_a_lookup_error(session):
    with pytest.raises(LookupError, match="account 99 not found"):
        withdraw(session, 99, 10)
