import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import Session

from candidate import Base, Product, on_sale_under

ROWS = [
    ("lamp", 999, 25),
    ("desk", 20000, 10),
    ("pen", 300, 0),
    ("mug", 1000, 50),
    ("chair", 1600, 50),
]


@pytest.fixture
def session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    with Session(engine) as s:
        s.add_all(Product(name=n, price_cents=p, discount_pct=d) for n, p, d in ROWS)
        s.commit()
        yield s


def test_sale_price_is_whole_cents_rounded_down():
    lamp = Product(name="lamp", price_cents=999, discount_pct=25)
    assert lamp.sale_price_cents == 749
    assert isinstance(lamp.sale_price_cents, int)


def test_query_filters_and_orders_on_the_sale_price(session):
    assert on_sale_under(session, 799) == ["mug", "lamp"]


def test_bound_is_inclusive_and_undiscounted_items_are_skipped(session):
    assert on_sale_under(session, 800) == ["mug", "lamp", "chair"]
    assert on_sale_under(session, 749) == ["mug", "lamp"]
    assert on_sale_under(session, 100000) == ["mug", "lamp", "chair", "desk"]


def test_discount_is_validated():
    with pytest.raises(ValueError, match="discount must be between 0 and 90"):
        Product(name="x", price_cents=100, discount_pct=95)
    with pytest.raises(ValueError, match="discount must be between 0 and 90"):
        Product(name="x", price_cents=100, discount_pct=-1)
