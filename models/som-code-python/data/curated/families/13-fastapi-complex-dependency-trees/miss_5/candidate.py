"""Tenant-scoped repositories built from a tree of shared FastAPI dependencies."""

from collections.abc import Generator

from fastapi import Depends, FastAPI, Header, HTTPException, status
from pydantic import BaseModel


class UserAuth(BaseModel):
    """The caller identified by an API key."""

    user_id: str
    role: str
    home_tenant: str


class TenantContext(BaseModel):
    """The tenant a request acts on, with the caller who chose it."""

    tenant_id: str
    user_id: str
    role: str
    home_tenant: str


class ScopedRepository:
    """A per-request repository bound to one tenant."""

    def __init__(self, tenant_id: str) -> None:
        self.tenant_id = tenant_id
        self.is_closed = False
        self.operations: list[str] = []

    def perform_action(self, action: str) -> str:
        """Record an action and return it qualified by the tenant."""
        if self.is_closed:
            raise RuntimeError("Repository is already closed")
        self.operations.append(action)
        return f"{self.tenant_id}:{action}"

    def close(self) -> None:
        """Refuse any further action."""
        self.is_closed = True


API_KEYS_DB: dict[str, dict[str, str]] = {
    "key-admin-1": {
        "user_id": "user-admin",
        "role": "admin",
        "home_tenant": "tenant-corp",
    },
    "key-member-1": {
        "user_id": "user-member",
        "role": "member",
        "home_tenant": "tenant-acme",
    },
}

OPEN_REPOSITORIES: list[ScopedRepository] = []


def reset_db() -> None:
    """Forget every repository opened so far."""
    OPEN_REPOSITORIES.clear()


def get_api_key(
    x_api_key: str | None = Header(default=None, alias="X-API-Key"),
) -> UserAuth:
    """Resolve the X-API-Key header to a user, or answer 401."""
    if not x_api_key or x_api_key not in API_KEYS_DB:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid or missing API key",
        )
    record = API_KEYS_DB[x_api_key]
    return UserAuth(
        user_id=record["user_id"],
        role=record["role"],
        home_tenant=record["home_tenant"],
    )


def get_tenant_context(
    auth: UserAuth = Depends(get_api_key),
    x_tenant_override: str | None = Header(default=None, alias="X-Tenant-Override"),
) -> TenantContext:
    """Choose the active tenant; only an admin may override it."""
    if x_tenant_override:
        if auth.role != "admin":
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="Tenant override requires admin privileges",
            )
        active_tenant = x_tenant_override
    else:
        active_tenant = auth.home_tenant

    return TenantContext(
        tenant_id=active_tenant,
        user_id=auth.user_id,
        role=auth.role,
        home_tenant=auth.home_tenant,
    )


def get_scoped_repository(
    tenant_ctx: TenantContext = Depends(get_tenant_context),
) -> Generator[ScopedRepository, None, None]:
    """Open one repository per request and close it after the response."""
    repo = ScopedRepository(tenant_id=tenant_ctx.tenant_id)
    OPEN_REPOSITORIES.append(repo)
    try:
        yield repo
    finally:
        repo.close()


class AuditService:
    """A service that shares the request's repository."""

    def __init__(self, repo: ScopedRepository) -> None:
        self.repo = repo


def get_audit_service(
    repo: ScopedRepository = Depends(get_scoped_repository, use_cache=False),
) -> AuditService:
    """Build the audit service on the request's repository."""
    return AuditService(repo=repo)


app = FastAPI(title="SaaS Scoped Dependency Service")


@app.post("/data/action")
def run_action(
    action: str = "query",
    repo: ScopedRepository = Depends(get_scoped_repository, use_cache=False),
    audit_service: AuditService = Depends(get_audit_service),
) -> dict[str, str | bool | int]:
    """Run an action and report whether both dependencies share a repository."""
    result = repo.perform_action(action)
    is_shared = repo is audit_service.repo
    return {
        "tenant_id": repo.tenant_id,
        "result": result,
        "shared_repo_instance": is_shared,
        "instance_count": len(OPEN_REPOSITORIES),
    }


@app.get("/tenants/current")
def get_current_tenant(
    tenant_ctx: TenantContext = Depends(get_tenant_context),
) -> dict[str, str]:
    """Return the tenant context of the request."""
    return {
        "tenant_id": tenant_ctx.tenant_id,
        "user_id": tenant_ctx.user_id,
        "role": tenant_ctx.role,
        "home_tenant": tenant_ctx.home_tenant,
    }
