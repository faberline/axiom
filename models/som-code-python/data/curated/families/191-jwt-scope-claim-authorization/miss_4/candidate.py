"""Authorize a call from the space-separated scope claim of a verified JWT."""

from collections.abc import Mapping
from typing import Any

import jwt


class InsufficientScopeError(Exception):
    """The token is valid but lacks some of the required scopes."""

    def __init__(self, missing: frozenset[str]) -> None:
        super().__init__("missing scopes: " + " ".join(sorted(missing)))
        self.missing = missing


def granted_scopes(claims: Mapping[str, Any]) -> frozenset[str]:
    """Return the scopes named in the claims; no scope claim grants nothing."""
    raw = claims["scope"]
    if not isinstance(raw, str):
        raise TypeError("scope claim must be a space-separated string")
    return frozenset(raw.split())


def require_scopes(token: str, secret: str, *needed: str) -> dict[str, Any]:
    """Verify the token and insist that every needed scope was granted."""
    if not needed:
        raise ValueError("at least one scope must be required")
    claims: dict[str, Any] = jwt.decode(token, secret, algorithms=["HS256"])
    missing = frozenset(needed) - granted_scopes(claims)
    if missing:
        raise InsufficientScopeError(missing)
    return claims
