"""Verify RS256 tokens against the signing key a JWKS document lists for their kid."""

from collections.abc import Mapping
from typing import Any

import jwt


class JwksVerifier:
    """Index the signing keys of a JWKS by kid and verify tokens against them."""

    def __init__(self, jwks: Mapping[str, Any], audience: str) -> None:
        key_set = jwt.PyJWKSet.from_dict(dict(jwks))
        self._keys = {
            key.key_id: key
            for key in key_set.keys
            if key.key_id and key.public_key_use in {None, "sig"}
        }
        if not self._keys:
            raise ValueError("JWKS contains no signing keys with a kid")
        self._audience = audience

    def verify(self, token: str) -> dict[str, Any]:
        """Pick the key named by the token's kid and verify with RS256 only."""
        kid = jwt.get_unverified_header(token).get("kid")
        key = self._keys.get(kid) if isinstance(kid, str) else None
        if key is None:
            raise jwt.InvalidTokenError(f"no signing key for kid {kid!r}")
        claims: dict[str, Any] = jwt.decode(
            token, key.key, algorithms=["RS256"], audience=self._audience
        )
        return claims
