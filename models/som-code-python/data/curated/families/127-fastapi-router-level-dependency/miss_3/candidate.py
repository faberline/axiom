"""Admin routes guarded once at the router instead of on every handler."""

from typing import Annotated

from fastapi import APIRouter, Depends, FastAPI, Header, HTTPException, Request, status

KEYS = {"admin-key-127": "admin", "viewer-key-127": "viewer"}
USERS: dict[int, str] = {}
AUDIT: list[str] = []


def reset_db() -> None:
    """Restore the seed users and clear the audit log."""
    USERS.clear()
    USERS.update({1: "bob", 2: "ada"})
    AUDIT.clear()


def require_admin(
    request: Request, x_api_key: Annotated[str | None, Header()] = None
) -> str:
    """Reject missing or unknown keys with 401 and non-admin roles with 403."""
    role = KEYS.get(x_api_key or "")
    if role is None:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, detail="invalid api key")
    AUDIT.append(f"{role}:{request.method} {request.url.path}")
    if role != "admin":
        raise HTTPException(status.HTTP_403_FORBIDDEN, detail="admin role required")
    return role


admin = APIRouter(prefix="/admin", dependencies=[Depends(require_admin)])
public = APIRouter(prefix="/public")


@admin.get("/users")
def list_users() -> list[str]:
    """Return every user name in alphabetical order."""
    return sorted(USERS.values())


@admin.delete("/users/{user_id}", status_code=status.HTTP_204_NO_CONTENT)
def delete_user(user_id: int) -> None:
    """Delete one user or answer 404."""
    if USERS.pop(user_id, None) is None:
        raise HTTPException(
            status.HTTP_404_NOT_FOUND, detail=f"user {user_id} not found"
        )


@public.get("/ping")
def ping() -> dict[str, bool]:
    """Answer without authentication."""
    return {"pong": True}


app = FastAPI(title="Admin console")
app.include_router(admin)
app.include_router(public)
reset_db()
