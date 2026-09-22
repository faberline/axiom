import pytest
from fastapi.testclient import TestClient
import candidate
from candidate import app


@pytest.fixture(autouse=True)
def clean_database():
    """Ensure database is fresh for every test in StaticPool in-memory SQLite."""
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()
    elif hasattr(candidate, "Base") and hasattr(candidate, "engine"):
        candidate.Base.metadata.drop_all(bind=candidate.engine)
        candidate.Base.metadata.create_all(bind=candidate.engine)
    yield


@pytest.fixture
def client():
    """Default client with per-request session lifecycle."""
    return TestClient(app)


@pytest.fixture
def shared_session_client():
    """Client overriding get_db with a shared session.
    
    Verifies that after an IntegrityError, db.rollback() was called.
    If db.rollback() was omitted (miss_3), SQLAlchemy places the session
    into an inactive state and subsequent operations raise PendingRollbackError (HTTP 500).
    """
    shared_session = candidate.SessionLocal()
    candidate.app.dependency_overrides[candidate.get_db] = lambda: shared_session
    with TestClient(app) as test_client:
        yield test_client
    candidate.app.dependency_overrides.clear()
    shared_session.close()


def test_duplicate_email_returns_409_conflict(client):
    res1 = client.post("/users", json={"email": "test@example.com", "username": "user1"})
    assert res1.status_code == 201

    res2 = client.post("/users", json={"email": "test@example.com", "username": "user2"})
    assert res2.status_code == 409
    assert res2.json()["detail"] == "Email already registered"


def test_duplicate_email_does_not_overwrite_existing_user(client):
    res1 = client.post("/users", json={"email": "original@example.com", "username": "original_name"})
    assert res1.status_code == 201
    user_id = res1.json()["id"]

    res2 = client.post("/users", json={"email": "original@example.com", "username": "impostor_name"})
    assert res2.status_code == 409

    get_res = client.get(f"/users/{user_id}")
    assert get_res.status_code == 200
    assert get_res.json()["username"] == "original_name"


def test_session_clean_after_conflict_allows_subsequent_registration(shared_session_client):
    # Seed user
    res1 = shared_session_client.post("/users", json={"email": "seeded@example.com", "username": "seed_user"})
    assert res1.status_code == 201

    # Conflict trigger
    conflict_res = shared_session_client.post("/users", json={"email": "seeded@example.com", "username": "duplicate_attempt"})
    assert conflict_res.status_code == 409

    # Subsequent registration within the shared session:
    # In gold: db.rollback() cleaned the session, returns 201.
    # In miss_3: missing db.rollback() leaves session in PendingRollbackError, returns 500!
    subsequent_res = shared_session_client.post("/users", json={"email": "unique@example.com", "username": "clean_user"})
    assert subsequent_res.status_code == 201
