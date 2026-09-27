"""Pull a JWT out of an Authorization header and turn every failure into a 401."""

from typing import Any

import jwt


class AuthError(Exception):
    """An authentication failure carrying the HTTP status to answer with."""

    def __init__(self, status: int, detail: str) -> None:
        super().__init__(detail)
        self.status = status
        self.detail = detail


def extract_bearer(header: str | None) -> str:
    """Return the token from a Bearer Authorization header value."""
    if header is None:
        raise AuthError(401, "missing authorization header")
    scheme, _, rest = header.strip().partition(" ")
    if scheme.lower() != "bearer":
        raise AuthError(401, "authorization scheme must be Bearer")
    token = rest.strip()
    if not token or " " in token:
        raise AuthError(401, "malformed bearer token")
    return token


def authenticate(header: str | None, secret: str, audience: str) -> dict[str, Any]:
    """Verify the bearer token for audience and return its claims."""
    token = extract_bearer(header)
    try:
        claims: dict[str, Any] = jwt.decode(
            token, secret, algorithms=["HS256"], audience=audience
        )
    except jwt.InvalidTokenError as exc:
        raise AuthError(403, "invalid token") from exc
    return claims
