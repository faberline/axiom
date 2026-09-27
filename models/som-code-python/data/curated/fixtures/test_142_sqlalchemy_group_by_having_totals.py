import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import Session

from candidate import Base, Sale, top_regions

SALES = [
    ("north", 100),
    ("north", 200),
    ("south", 500),
    ("east", 150),
    ("east", 150),
    ("west", 50),
]


@pytest.fixture
def session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    with Session(engine) as s:
        s.add_all(Sale(region=r, amount_cents=a) for r, a in SALES)
        s.commit()
        yield s


def test_totals_are_grouped_and_ties_break_by_region(session):
    assert top_regions(session, min_total=100) == [
        ("south", 500, 1),
        ("east", 300, 2),
        ("north", 300, 2),
    ]


def test_limit_caps_the_result(session):
    assert top_regions(session, min_total=0, limit=2) == [
        ("south", 500, 1),
        ("east", 300, 2),
    ]


def test_threshold_applies_to_the_group_total_inclusively(session):
    assert [r for r, _, _ in top_regions(session, min_total=300)] == [
        "south",
        "east",
        "north",
    ]
    assert top_regions(session, min_total=301) == [("south", 500, 1)]


def test_limit_must_be_positive(session):
    with pytest.raises(ValueError, match="limit must be at least 1"):
        top_regions(session, min_total=0, limit=0)
