"""Keep access and refresh JWTs apart with a typ claim and separate lifetimes."""

import datetime as dt
from typing import Any

import jwt

ALGORITHM = "HS256"
ACCESS_TTL = dt.timedelta(minutes=15)
REFRESH_TTL = dt.timedelta(days=7)


class WrongTokenTypeError(jwt.InvalidTokenError):
    """A token of one type was presented where the other type was required."""


def _encode(secret: str, subject: str, typ: str, ttl: dt.timedelta) -> str:
    now = dt.datetime.now(dt.UTC)
    claims = {"sub": subject, "typ": typ, "iat": now, "exp": now + ttl}
    return jwt.encode(claims, secret, algorithm=ALGORITHM)


def issue_pair(secret: str, subject: str) -> tuple[str, str]:
    """Return a short-lived access token and a long-lived refresh token."""
    if not subject:
        raise ValueError("subject must not be empty")
    access = _encode(secret, subject, "access", ACCESS_TTL)
    refresh = _encode(secret, subject, "refresh", REFRESH_TTL)
    return access, refresh


def read_token(secret: str, token: str, expected_typ: str) -> dict[str, Any]:
    """Verify the token and insist that its typ claim is the expected one."""
    claims: dict[str, Any] = jwt.decode(
        token,
        secret,
        algorithms=[ALGORITHM],
        options={"require": ["sub", "typ", "exp"]},
    )
    if claims["typ"] != expected_typ:
        raise WrongTokenTypeError(f"expected a {expected_typ} token")
    return claims


def refresh_access(secret: str, refresh_token: str) -> str:
    """Trade a valid refresh token for a new access token for the same subject."""
    claims = read_token(secret, refresh_token, "refresh")
    return _encode(secret, str(claims["sub"]), "access", ACCESS_TTL)
