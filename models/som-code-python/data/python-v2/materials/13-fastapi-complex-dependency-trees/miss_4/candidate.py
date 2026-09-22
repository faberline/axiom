from typing import Any, Dict, Generator, List, Optional
from fastapi import Depends, FastAPI, Header, HTTPException, status
from pydantic import BaseModel


class UserAuth(BaseModel):
    user_id: str
    role: str
    home_tenant: str


class TenantContext(BaseModel):
    tenant_id: str
    user_id: str
    role: str
    home_tenant: str


class ScopedRepository:
    def __init__(self, tenant_id: str):
        self.tenant_id = tenant_id
        self.is_closed = False
        self.operations: List[str] = []

    def perform_action(self, action: str) -> str:
        if self.is_closed:
            raise RuntimeError("Repository is already closed")
        self.operations.append(action)
        return f"{self.tenant_id}:{action}"

    def close(self) -> None:
        self.is_closed = True


API_KEYS_DB: Dict[str, Dict[str, str]] = {
    "key-admin-1": {"user_id": "user-admin", "role": "admin", "home_tenant": "tenant-corp"},
    "key-member-1": {"user_id": "user-member", "role": "member", "home_tenant": "tenant-acme"},
}

OPEN_REPOSITORIES: List[ScopedRepository] = []


def reset_db() -> None:
    global OPEN_REPOSITORIES
    OPEN_REPOSITORIES = []


def get_api_key(
    x_api_key: Optional[str] = Header(default=None, alias="X-API-Key"),
) -> UserAuth:
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
    x_tenant_override: Optional[str] = Header(default=None, alias="X-Tenant-Override"),
) -> TenantContext:
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
) -> ScopedRepository:
    repo = ScopedRepository(tenant_id=tenant_ctx.tenant_id)
    OPEN_REPOSITORIES.append(repo)
    return repo

class AuditService:
    def __init__(self, repo: ScopedRepository):
        self.repo = repo


def get_audit_service(
    repo: ScopedRepository = Depends(get_scoped_repository),
) -> AuditService:
    return AuditService(repo=repo)


app = FastAPI(title="SaaS Scoped Dependency Service")


@app.post("/data/action")
def run_action(
    action: str = "query",
    repo: ScopedRepository = Depends(get_scoped_repository),
    audit_service: AuditService = Depends(get_audit_service),
):
    result = repo.perform_action(action)
    is_shared = (repo is audit_service.repo)
    return {
        "tenant_id": repo.tenant_id,
        "result": result,
        "shared_repo_instance": is_shared,
        "instance_count": len(OPEN_REPOSITORIES),
    }


@app.get("/tenants/current")
def get_current_tenant(
    tenant_ctx: TenantContext = Depends(get_tenant_context),
):
    return {
        "tenant_id": tenant_ctx.tenant_id,
        "user_id": tenant_ctx.user_id,
        "role": tenant_ctx.role,
        "home_tenant": tenant_ctx.home_tenant,
    }
