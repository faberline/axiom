import pytest
from fastapi.testclient import TestClient
import candidate
from candidate import app


@pytest.fixture(autouse=True)
def clean_database():
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()
    yield


def test_access_own_tenant_document_success_200():
    client = TestClient(app)
    # Legitimate tenant accessing their own document must succeed with 200 (catches miss_5)
    res = client.get("/documents/1", headers={"X-Tenant-ID": "tenant-alpha"})
    assert res.status_code == 200, f"Expected 200 for tenant-alpha, got {res.status_code}"
    data = res.json()
    assert data["id"] == 1
    assert data["tenant_id"] == "tenant-alpha"
    assert data["title"] == "Alpha Strategy"
    assert data["content"] == "Classified Alpha"


def test_access_second_tenant_document_success_200():
    client = TestClient(app)
    # Legitimate second tenant accessing their own document 2 must succeed with 200 and return proper content
    # Closes Challenger 1 tenant symmetry gap where candidate hardcodes tenant-alpha
    res = client.get("/documents/2", headers={"X-Tenant-ID": "tenant-beta"})
    assert res.status_code == 200, f"Expected 200 for tenant-beta, got {res.status_code}"
    data = res.json()
    assert data["id"] == 2
    assert data["tenant_id"] == "tenant-beta"
    assert data["title"] == "Beta Plans"
    assert data["content"] == "Classified Beta"


def test_access_other_tenant_document_forbidden_403():
    client = TestClient(app)
    # Accessing another tenant's document must return 403 Forbidden (catches miss_1 IDOR and miss_3 401)
    res = client.get("/documents/1", headers={"X-Tenant-ID": "tenant-beta"})
    assert res.status_code == 403, f"Expected 403 for cross-tenant access, got {res.status_code}"
    assert res.json().get("detail") == "Forbidden: Tenant mismatch"


def test_missing_tenant_header_rejected_422():
    client = TestClient(app)
    # Required header X-Tenant-ID must be enforced with 422 (catches miss_2 optional header)
    res = client.get("/documents/1")
    assert res.status_code == 422, f"Expected 422 for missing X-Tenant-ID, got {res.status_code}"


def test_nonexistent_document_returns_404_even_with_valid_header():
    client = TestClient(app)
    # Non-existent document ID must return 404 Not Found, not 403 Forbidden (catches miss_4)
    res = client.get("/documents/999", headers={"X-Tenant-ID": "tenant-alpha"})
    assert res.status_code == 404, f"Expected 404 for non-existent document, got {res.status_code}"
    assert res.json().get("detail") == "Document not found"
