"""Rotate ES256 signing keys by kid while tokens signed earlier keep verifying."""

from collections.abc import Mapping
from typing import Any

import jwt
from cryptography.hazmat.primitives.asymmetric import ec


class UnknownKeyError(jwt.InvalidTokenError):
    """The token's kid header names no key in the ring."""


class KeyRing:
    """Signing keys by kid; the newest key signs, every kept key verifies."""

    def __init__(self) -> None:
        self._keys: dict[str, ec.EllipticCurvePrivateKey] = {}
        self.active_kid: str | None = None

    def rotate(self, kid: str) -> None:
        """Generate a new P-256 key under kid and make it the signing key."""
        if not kid or kid in self._keys:
            raise ValueError(f"kid must be new and non-empty: {kid!r}")
        self._keys[kid] = ec.generate_private_key(ec.SECP256R1())
        self.active_kid = kid

    def retire(self, kid: str) -> None:
        """Drop an old key so tokens signed with it stop verifying."""
        del self._keys[kid]

    def sign(self, claims: Mapping[str, Any]) -> str:
        """Sign claims with the active key, naming it in the kid header."""
        if self.active_kid is None:
            raise LookupError("no active signing key")
        key = self._keys[self.active_kid]
        return jwt.encode(
            dict(claims), key, algorithm="ES256", headers={"kid": self.active_kid}
        )

    def verify(self, token: str) -> dict[str, Any]:
        """Verify with the public half of the key the kid header names."""
        kid = jwt.get_unverified_header(token).get("kid")
        if not isinstance(kid, str) or kid not in self._keys:
            raise UnknownKeyError(f"unknown kid: {kid!r}")
        claims: dict[str, Any] = jwt.decode(
            token, self._keys[kid].public_key(), algorithms=["ES256"]
        )
        return claims
