from datetime import datetime, timedelta

import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import Session

from candidate import Base, Event, page_after


@pytest.fixture
def session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    t0 = datetime(2026, 1, 1)
    with Session(engine) as s:
        # ids 1..7; ids 3,4,5 share one timestamp
        stamps = [0, 1, 2, 2, 2, 3, 4]
        s.add_all(Event(name=f"e{i + 1}", created=t0 + timedelta(minutes=m))
                  for i, m in enumerate(stamps))
        s.commit()
        yield s


def names(events):
    return [e.name for e in events]


def test_first_page_newest_first(session):
    events, cursor = page_after(session, None, size=3)
    assert names(events) == ["e7", "e6", "e5"]
    assert cursor == (events[-1].created, 5)


def test_walk_all_pages_without_gaps_or_duplicates(session):
    seen, cursor = [], None
    while True:
        events, cursor = page_after(session, cursor, size=2)
        seen += names(events)
        if cursor is None:
            break
    assert seen == ["e7", "e6", "e5", "e4", "e3", "e2", "e1"]


def test_last_page_has_no_cursor(session):
    _, cursor = page_after(session, None, size=7)
    assert cursor is None
    events, cursor = page_after(session, None, size=10)
    assert len(events) == 7 and cursor is None


def test_size_is_bounded(session):
    with pytest.raises(ValueError, match="size must be between 1 and 100"):
        page_after(session, None, size=0)
    with pytest.raises(ValueError, match="size must be between 1 and 100"):
        page_after(session, None, size=101)
