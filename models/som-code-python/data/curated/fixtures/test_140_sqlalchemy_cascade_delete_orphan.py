import pytest
from sqlalchemy import create_engine, func, select
from sqlalchemy.orm import Session

from candidate import Base, Line, Order, delete_order, replace_lines


@pytest.fixture
def session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    with Session(engine) as s:
        order = Order(customer="ada")
        order.lines = [Line(sku="A", qty=1), Line(sku="B", qty=2)]
        s.add_all([order, Order(customer="bob", lines=[Line(sku="Z", qty=9)])])
        s.commit()
        yield s


def line_count(session):
    return session.scalar(select(func.count()).select_from(Line))


def test_replacing_lines_deletes_the_old_rows(session):
    order = replace_lines(session, 1, [("C", 3), ("D", 4), ("E", 5)])
    assert [(line.sku, line.qty) for line in order.lines] == [
        ("C", 3),
        ("D", 4),
        ("E", 5),
    ]
    assert line_count(session) == 4


def test_deleting_an_order_deletes_its_lines(session):
    delete_order(session, 1)
    session.rollback()
    assert session.get(Order, 1) is None
    assert line_count(session) == 1


def test_bad_quantity_changes_nothing(session):
    order = session.get(Order, 1)
    with pytest.raises(ValueError, match="quantity for D must be at least 1"):
        replace_lines(session, 1, [("C", 1), ("D", 0)])
    assert [line.sku for line in order.lines] == ["A", "B"]
    assert line_count(session) == 3


def test_unknown_orders_raise(session):
    with pytest.raises(LookupError, match="order 7 not found"):
        replace_lines(session, 7, [])
    with pytest.raises(LookupError, match="order 7 not found"):
        delete_order(session, 7)
