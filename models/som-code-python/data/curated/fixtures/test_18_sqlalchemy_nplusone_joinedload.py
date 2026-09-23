import candidate
from fastapi.testclient import TestClient
import pytest


@pytest.fixture(autouse=True)
def clean_database():
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()
    elif hasattr(candidate, "Base") and hasattr(candidate, "engine"):
        candidate.Base.metadata.drop_all(bind=candidate.engine)
        candidate.Base.metadata.create_all(bind=candidate.engine)
    yield


@pytest.fixture
def client():
    return TestClient(candidate.app)


def test_articles_query_count_strictly_one(client):
    candidate.QueryCounter.reset()
    res = client.get("/articles")
    assert res.status_code == 200
    assert candidate.QueryCounter.count == 1


def test_articles_preserves_null_author_articles(client):
    res = client.get("/articles")
    assert res.status_code == 200
    articles = res.json()
    assert len(articles) == 4
    null_authors = [a for a in articles if a["author"] is None]
    assert len(null_authors) == 1
    assert null_authors[0]["title"] == "Anonymous Editorial"


def test_articles_boundary_all_returned(client):
    res = client.get("/articles")
    assert res.status_code == 200
    articles = res.json()
    assert len(articles) == 4
    assert articles[0]["title"] == "Deep Dive into Async"
    assert articles[-1]["title"] == "Anonymous Editorial"


def test_articles_author_data_integrity(client):
    res = client.get("/articles")
    assert res.status_code == 200
    articles = res.json()
    first = articles[0]
    assert first["author"] is not None
    assert first["author"]["name"] == "Alice Smith"
    assert first["author"]["email"] == "alice@example.com"
