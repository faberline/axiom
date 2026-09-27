"""Verify JWTs from several trusted issuers, each with its own HMAC secret."""

from collections.abc import Mapping
from typing import Any

import jwt


class UntrustedIssuerError(jwt.InvalidTokenError):
    """The token names an issuer that has no configured secret."""


def issue(secret: str, issuer: str, subject: str, audience: str) -> str:
    """Sign a token the way a trusted issuer would."""
    claims = {"iss": issuer, "sub": subject, "aud": audience}
    return jwt.encode(claims, secret, algorithm="HS256")


def verify(token: str, secrets: Mapping[str, str], audience: str) -> dict[str, Any]:
    """Pick the secret by the unverified issuer, then fully verify with it."""
    unverified = jwt.decode(token, options={"verify_signature": False})
    issuer = unverified.get("iss")
    if not isinstance(issuer, str) or issuer not in secrets:
        raise UntrustedIssuerError(f"untrusted issuer: {issuer!r}")
    claims: dict[str, Any] = jwt.decode(
        token,
        secrets[issuer],
        algorithms=["HS256"],
        audience=audience,
        issuer=issuer,
        options={"require": ["iss", "aud"]},
    )
    return claims
