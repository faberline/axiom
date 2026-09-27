import pytest
from sqlalchemy import create_engine, select
from sqlalchemy.orm import Session

from candidate import Base, Stock, record_counts


@pytest.fixture
def session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    with Session(engine) as s:
        s.add_all([Stock(sku="A", qty=5, updates=1), Stock(sku="B", qty=7, updates=3)])
        s.commit()
        yield s


def rows(session):
    stmt = select(Stock.sku, Stock.qty, Stock.updates).order_by(Stock.sku)
    return [tuple(row) for row in session.execute(stmt)]


def test_new_skus_are_inserted_and_existing_ones_replaced(session):
    assert record_counts(session, {"A": 2, "C": 0}) == 2
    session.rollback()
    assert rows(session) == [("A", 2, 2), ("B", 7, 3), ("C", 0, 1)]


def test_repeated_counts_keep_bumping_updates(session):
    record_counts(session, {"B": 1})
    record_counts(session, {"B": 4})
    assert rows(session)[1] == ("B", 4, 5)


def test_negative_counts_write_nothing(session):
    with pytest.raises(ValueError, match="count for C must not be negative"):
        record_counts(session, {"A": 1, "C": -1})
    assert rows(session) == [("A", 5, 1), ("B", 7, 3)]


def test_empty_input_is_a_no_op(session):
    assert record_counts(session, {}) == 0
    assert rows(session) == [("A", 5, 1), ("B", 7, 3)]
