from datetime import datetime

import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import Session

import candidate
from candidate import Article, Base, edit

T1 = datetime(2026, 1, 1, 9, 0)
T2 = datetime(2026, 1, 2, 9, 0)
T3 = datetime(2026, 1, 3, 9, 0)


@pytest.fixture
def session(monkeypatch):
    times = iter([T1, T2, T3])
    monkeypatch.setattr(candidate, "now", lambda: next(times))
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    with Session(engine) as s:
        s.add(Article(id=1, title="Draft", body="hello"))
        s.commit()
        yield s
    engine.dispose()


def test_insert_stamps_both_times_and_starts_at_revision_one(session):
    article = session.get(Article, 1)
    assert article.created_at == T1
    assert article.updated_at == T1
    assert article.revision == 1


def test_real_change_updates_the_stamp_and_revision(session):
    article = session.get(Article, 1)
    edit(session, article, title="Final")
    assert article.title == "Final"
    assert article.created_at == T1
    assert article.updated_at == T2
    assert article.revision == 2


def test_setting_the_same_value_is_not_a_revision(session):
    article = session.get(Article, 1)
    edit(session, article, title="Draft", body="hello")
    assert article.updated_at == T1
    assert article.revision == 1


def test_body_change_is_a_revision(session):
    article = session.get(Article, 1)
    edit(session, article, body="hello world")
    edit(session, article, body="hello again")
    assert article.updated_at == T3
    assert article.revision == 3


def test_blank_title_is_rejected_and_nothing_changes(session):
    article = session.get(Article, 1)
    with pytest.raises(ValueError, match="title must not be blank"):
        edit(session, article, title="   ")
    session.rollback()
    assert article.title == "Draft"
    assert article.revision == 1
