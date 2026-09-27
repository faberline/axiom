from datetime import UTC, datetime

import pytest
from sqlalchemy import create_engine, func, select
from sqlalchemy.orm import Session

from candidate import Base, Note, active_notes, restore, soft_delete

T = datetime(2026, 3, 1, 12, 0, tzinfo=UTC)


@pytest.fixture
def session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    with Session(engine) as s:
        s.add_all([Note(title="b"), Note(title="a"), Note(title="c")])
        s.commit()
        yield s


def test_soft_delete_hides_but_keeps_the_row(session):
    assert soft_delete(session, 1, now=T) is True
    assert [n.title for n in active_notes(session)] == ["a", "c"]
    assert session.scalar(select(func.count()).select_from(Note)) == 3
    assert session.get(Note, 1).deleted_at is not None


def test_deleting_twice_keeps_the_first_timestamp(session):
    soft_delete(session, 1, now=T)
    later = T.replace(hour=13)
    assert soft_delete(session, 1, now=later) is False
    stamp = session.get(Note, 1).deleted_at
    assert stamp.replace(tzinfo=UTC) == T


def test_restore_brings_it_back(session):
    soft_delete(session, 2, now=T)
    assert restore(session, 2) is True
    assert [n.title for n in active_notes(session)] == ["a", "b", "c"]
    assert restore(session, 2) is False


def test_unknown_ids_raise(session):
    with pytest.raises(LookupError, match="note 9 not found"):
        soft_delete(session, 9, now=T)
    with pytest.raises(LookupError, match="note 9 not found"):
        restore(session, 9)


def test_changes_are_committed(session):
    soft_delete(session, 3, now=T)
    session.rollback()
    assert [n.title for n in active_notes(session)] == ["a", "b"]
