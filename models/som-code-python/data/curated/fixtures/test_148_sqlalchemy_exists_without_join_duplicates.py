import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import Session

from candidate import (
    Base,
    Customer,
    Invoice,
    customers_owing,
    customers_without_invoices,
    has_unpaid,
)


@pytest.fixture
def session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    with Session(engine) as s:
        s.add_all(
            [
                Customer(id=1, name="ann"),
                Customer(id=2, name="bob"),
                Customer(id=3, name="cy"),
                Customer(id=4, name="dee"),
                Invoice(customer_id=1, amount_cents=500),
                Invoice(customer_id=1, amount_cents=700),
                Invoice(customer_id=1, amount_cents=100, paid=True),
                Invoice(customer_id=2, amount_cents=300, paid=True),
                Invoice(customer_id=3, amount_cents=200),
            ]
        )
        s.commit()
        yield s
    engine.dispose()


def test_has_unpaid_is_a_real_bool(session):
    assert has_unpaid(session, 1) is True
    assert has_unpaid(session, 2) is False
    assert has_unpaid(session, 4) is False


def test_owing_lists_each_customer_once(session):
    assert customers_owing(session, 0) == ["ann", "cy"]


def test_owing_threshold_is_inclusive(session):
    assert customers_owing(session, 200) == ["ann", "cy"]
    assert customers_owing(session, 250) == ["ann"]
    assert customers_owing(session, 701) == []


def test_negative_threshold_is_rejected(session):
    with pytest.raises(ValueError, match="min_cents must not be negative"):
        customers_owing(session, -1)


def test_customers_without_invoices(session):
    assert customers_without_invoices(session) == ["dee"]
