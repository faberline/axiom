import pytest
from sqlalchemy import create_engine, text

from candidate import escape_like, search_users

EMAILS = ["a_b@x.io", "axb@x.io", "zed@example.com", "100%@deals.io", "1000@deals.io"]


@pytest.fixture
def conn():
    engine = create_engine("sqlite://")
    with engine.connect() as c:
        c.execute(text("CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT)"))
        for email in EMAILS:
            c.execute(text("INSERT INTO users (email) VALUES (:e)"), {"e": email})
        yield c


def emails(rows):
    return [email for _, email in rows]


def test_matches_are_ordered_and_carry_ids(conn):
    assert search_users(conn, "x.io") == [(1, "a_b@x.io"), (2, "axb@x.io")]


def test_wildcards_in_the_term_are_literal(conn):
    assert emails(search_users(conn, "a_b")) == ["a_b@x.io"]
    assert emails(search_users(conn, "100%")) == ["100%@deals.io"]


def test_escape_like_escapes_backslash_first():
    assert escape_like("a\\_%") == "a\\\\\\_\\%"


def test_injection_is_just_a_string(conn):
    assert search_users(conn, "' OR 1=1 --") == []
    assert len(search_users(conn, "@", limit=50)) == 5


def test_limit_and_blank_terms(conn):
    assert len(search_users(conn, "@", limit=2)) == 2
    with pytest.raises(ValueError, match="limit must be between 1 and 50"):
        search_users(conn, "@", limit=51)
    with pytest.raises(ValueError, match="limit must be between 1 and 50"):
        search_users(conn, "@", limit=0)
    with pytest.raises(ValueError, match="search term must not be blank"):
        search_users(conn, "  ")
