import pytest
from fastapi.testclient import TestClient
import candidate
from candidate import app


@pytest.fixture(autouse=True)
def reset_candidate_db():
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()
    yield
    if hasattr(candidate, "reset_db"):
        candidate.reset_db()


def test_scoped_repository_closed_in_finally_cleanup():
    client = TestClient(app)
    response = client.post(
        "/data/action",
        headers={"X-API-Key": "key-admin-1"},
    )
    assert response.status_code == 200
    assert len(candidate.OPEN_REPOSITORIES) >= 1, "Expected at least one repository to be created"
    last_repo = candidate.OPEN_REPOSITORIES[-1]
    # Verifies generator teardown runs repo.close() (catches miss_1 omitting finally: repo.close())
    assert last_repo.is_closed is True, (
        "Repository instance was not closed after request completed; generator cleanup failed"
    )


def test_non_admin_cannot_override_tenant_403():
    client = TestClient(app)
    # Member user attempts to override tenant context
    response = client.get(
        "/tenants/current",
        headers={
            "X-API-Key": "key-member-1",
            "X-Tenant-Override": "tenant-unauthorized-target",
        },
    )
    # Verifies non-admin role is denied tenant override (catches miss_2 allowing non-admin override)
    assert response.status_code == 403, (
        f"Expected 403 Forbidden for non-admin tenant override, got {response.status_code}"
    )


def test_admin_override_sets_override_tenant():
    client = TestClient(app)
    target_tenant = "tenant-audited-partner"
    response = client.get(
        "/tenants/current",
        headers={
            "X-API-Key": "key-admin-1",
            "X-Tenant-Override": target_tenant,
        },
    )
    assert response.status_code == 200
    data = response.json()
    # Verifies active tenant matches override header (catches miss_3 which sets home_tenant instead)
    assert data["tenant_id"] == target_tenant, (
        f"Expected tenant_id '{target_tenant}', got '{data['tenant_id']}'"
    )
    assert data["home_tenant"] == "tenant-corp"


def test_get_scoped_repository_is_generator_dependency():
    # Verifies get_scoped_repository is implemented as a generator function with yield (co_flags & 0x20)
    # Catches miss_4 which uses return repo directly instead of yield repo
    is_generator = bool(candidate.get_scoped_repository.__code__.co_flags & 0x20)
    assert is_generator is True, (
        "get_scoped_repository must be a generator function (using yield) to provide request lifecycle teardown"
    )


def test_dependency_caching_shares_repo_instance():
    client = TestClient(app)
    response = client.post(
        "/data/action",
        headers={"X-API-Key": "key-member-1"},
    )
    assert response.status_code == 200
    data = response.json()
    # Verifies dependency caching (use_cache=True) reuses single instance across dependents
    # Catches miss_5 which disables dependency caching (use_cache=False) causing duplicate instances
    assert data["shared_repo_instance"] is True, (
        "Expected route handler and dependent service to share the same ScopedRepository instance"
    )
    assert data["instance_count"] == 1, (
        f"Expected exactly 1 repository instance per request, got {data['instance_count']}"
    )


def test_missing_or_invalid_api_key_401():
    client = TestClient(app)
    r1 = client.get("/tenants/current")
    assert r1.status_code == 401

    r2 = client.get("/tenants/current", headers={"X-API-Key": "unknown-token"})
    assert r2.status_code == 401
