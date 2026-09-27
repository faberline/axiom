import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import Session

from candidate import Base, Score, leaderboard


@pytest.fixture
def session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    rows = [
        ("chess", "eve", 60),
        ("chess", "bob", 90),
        ("chess", "dee", 70),
        ("chess", "ann", 90),
        ("chess", "cy", 80),
        ("go", "bob", 40),
        ("go", "ann", 50),
    ]
    with Session(engine) as s:
        s.add_all(Score(game=g, player=p, points=n) for g, p, n in rows)
        s.commit()
        yield s
    engine.dispose()


def test_ties_share_a_place_and_skip_the_next(session):
    assert leaderboard(session) == {
        "chess": [("ann", 1), ("bob", 1), ("cy", 3)],
        "go": [("ann", 1), ("bob", 2)],
    }


def test_places_are_ranked_within_each_game(session):
    assert leaderboard(session, top=1) == {
        "chess": [("ann", 1), ("bob", 1)],
        "go": [("ann", 1)],
    }


def test_top_includes_the_last_place(session):
    board = leaderboard(session, top=5)
    assert board["chess"][-1] == ("eve", 5)


@pytest.mark.parametrize("top", [0, -1])
def test_top_must_be_positive(session, top):
    with pytest.raises(ValueError, match="top must be at least 1"):
        leaderboard(session, top=top)
