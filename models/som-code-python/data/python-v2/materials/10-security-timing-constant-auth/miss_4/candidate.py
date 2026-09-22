import hmac
import time
from typing import Annotated
from fastapi import Depends, FastAPI, Header, HTTPException, status
from pydantic import BaseModel

SECRET_KEY = b"som-crypto-secret-key-32bytes!!"


class AuthenticatedUser(BaseModel):
    user_id: str
    scopes: list[str]
    expires_at: int


def create_token(user_id: str, scopes: list[str], ttl_seconds: int = 3600) -> str:
    exp = int(time.time()) + ttl_seconds
    scopes_str = ",".join(scopes)
    payload = f"{user_id}:{scopes_str}:{exp}"
    signature = hmac.new(SECRET_KEY, payload.encode("utf-8"), "sha256").hexdigest()
    return f"{payload}:{signature}"


def verify_token(raw_header: str | None, required_scope: str | None = None) -> AuthenticatedUser:
    if not raw_header or not raw_header.startswith("Bearer "):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Missing or malformed Bearer authorization header",
        )
    token = raw_header[6:]
    parts = token.split(":")
    if len(parts) != 4:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid token payload segments",
        )
    user_id, scopes_str, exp_str, provided_sig = parts
    try:
        exp = int(exp_str)
    except ValueError:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Malformed expiration timestamp",
        )

    payload = f"{user_id}:{scopes_str}:{exp_str}"
    expected_sig = hmac.new(SECRET_KEY, payload.encode("utf-8"), "sha256").hexdigest()

    if not hmac.compare_digest(provided_sig, expected_sig):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid signature: cryptographic verification failed",
        )

    if exp < int(time.time()):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Token has expired",
        )

    user_scopes = scopes_str.split(",") if scopes_str else []
    if required_scope is not None and required_scope not in user_scopes:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Forbidden: insufficient permission scope",
        )

    return AuthenticatedUser(user_id=user_id, scopes=user_scopes, expires_at=exp)


app = FastAPI(title="Security Constant-Time Service")


def require_read(authorization: Annotated[str | None, Header(alias="Authorization")] = None) -> AuthenticatedUser:
    return verify_token(authorization, required_scope="read")


def require_admin(authorization: Annotated[str | None, Header(alias="Authorization")] = None) -> AuthenticatedUser:
    return verify_token(authorization, required_scope="admin")


@app.get("/api/reports")
def get_reports(user: AuthenticatedUser = Depends(require_read)):
    return {"status": "ok", "user": user.user_id, "data": "classified_reports"}


@app.post("/api/admin/purge")
def purge_system(user: AuthenticatedUser = Depends(require_admin)):
    return {"status": "ok", "user": user.user_id, "action": "purged"}
