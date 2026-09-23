import candidate
from candidate import app
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
    return TestClient(app)


def test_patch_partial_update_preserves_unset_fields(client):
    create_res = client.post(
        "/items",
        json={"name": "Widget", "description": "Original description", "price": 49.99},
    )
    assert create_res.status_code == 201
    item_id = create_res.json()["id"]

    patch_res = client.patch(
        f"/items/{item_id}",
        json={"name": "Updated Widget"},
    )
    assert patch_res.status_code == 200
    data = patch_res.json()
    assert data["name"] == "Updated Widget"
    assert data["description"] == "Original description"
    assert data["price"] == 49.99

    get_res = client.get(f"/items/{item_id}")
    assert get_res.status_code == 200
    get_data = get_res.json()
    assert get_data["name"] == "Updated Widget"
    assert get_data["description"] == "Original description"
    assert get_data["price"] == 49.99


def test_patch_explicit_null_clears_nullable_description(client):
    create_res = client.post(
        "/items",
        json={"name": "Gadget", "description": "Clearable note", "price": 10.0},
    )
    assert create_res.status_code == 201
    item_id = create_res.json()["id"]

    patch_res = client.patch(
        f"/items/{item_id}",
        json={"description": None},
    )
    assert patch_res.status_code == 200
    data = patch_res.json()
    assert data["description"] is None
    assert data["name"] == "Gadget"

    get_res = client.get(f"/items/{item_id}")
    assert get_res.status_code == 200
    assert get_res.json()["description"] is None


def test_patch_persists_changes_to_database(client):
    create_res = client.post(
        "/items",
        json={"name": "Persistent Item", "description": "Keep", "price": 100.0},
    )
    assert create_res.status_code == 201
    item_id = create_res.json()["id"]

    patch_res = client.patch(
        f"/items/{item_id}",
        json={"price": 125.50},
    )
    assert patch_res.status_code == 200

    get_res = client.get(f"/items/{item_id}")
    assert get_res.status_code == 200
    assert get_res.json()["price"] == 125.50


def test_patch_nonexistent_item_returns_404(client):
    patch_res = client.patch(
        "/items/99999",
        json={"name": "Ghost Item"},
    )
    assert patch_res.status_code == 404
