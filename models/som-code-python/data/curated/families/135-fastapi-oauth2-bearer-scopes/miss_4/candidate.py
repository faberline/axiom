"""Bearer-token API where each route declares the OAuth2 scopes it needs."""

from dataclasses import dataclass
from typing import Annotated

from fastapi import Depends, FastAPI, HTTPException, Security, status
from fastapi.security import OAuth2PasswordBearer, SecurityScopes


@dataclass(frozen=True)
class Principal:
    """The caller behind a token and the scopes it was granted."""

    name: str
    scopes: frozenset[str]


TOKENS = {
    "reader-token": Principal("rita", frozenset({"items:read"})),
    "writer-token": Principal("wes", frozenset({"items:read", "items:write"})),
    "write-only-token": Principal("wally", frozenset({"items:write"})),
}

oauth2 = OAuth2PasswordBearer(
    tokenUrl="token",
    scopes={"items:read": "Read items", "items:write": "Delete items"},
    auto_error=False,
)
app = FastAPI(title="Items")


def current_principal(
    security_scopes: SecurityScopes, token: Annotated[str | None, Depends(oauth2)]
) -> Principal:
    """Resolve the token, then require every scope the route declared."""
    challenge = f'Bearer scope="{security_scopes.scope_str}"'
    principal = TOKENS.get(token or "")
    if principal is None:
        raise HTTPException(
            status.HTTP_401_UNAUTHORIZED,
            detail="invalid token",
            headers={"WWW-Authenticate": challenge},
        )
    for scope in security_scopes.scopes:
        if scope not in principal.scopes:
            raise HTTPException(
                status.HTTP_403_FORBIDDEN,
                detail=f"missing scope {scope}",
                headers={
                    "WWW-Authenticate": 'Bearer error="insufficient_scope", '
                    f'scope="{security_scopes.scope_str}"'
                },
            )
    return principal


@app.get("/items")
def list_items(
    who: Annotated[Principal, Security(current_principal)],
) -> dict[str, str | list[str]]:
    """List items for any reader."""
    return {"user": who.name, "items": ["a", "b"]}


@app.delete("/items/{item_id}")
def delete_item(
    item_id: str,
    who: Annotated[
        Principal, Security(current_principal, scopes=["items:read", "items:write"])
    ],
) -> dict[str, str]:
    """Delete an item; needs read and write scopes."""
    return {"deleted": item_id, "by": who.name}
