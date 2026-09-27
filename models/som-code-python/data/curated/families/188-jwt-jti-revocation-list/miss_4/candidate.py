"""Revoke individual JWTs by jti and forget each revocation once the token expires."""

import datetime as dt
import uuid
from typing import Any

import jwt


class RevokedTokenError(jwt.InvalidTokenError):
    """The token's jti is on the revocation list."""


class TokenService:
    """Issue HS256 tokens with a unique jti and keep a revocation list."""

    def __init__(self, secret: str, ttl: dt.timedelta) -> None:
        if len(secret.encode()) < 32:
            raise ValueError("secret must be at least 32 bytes")
        self._secret = secret
        self._ttl = ttl
        self._revoked: dict[str, int] = {}

    @property
    def revoked_count(self) -> int:
        """Number of revocations currently remembered."""
        return len(self._revoked)

    def issue(self, subject: str) -> str:
        """Sign a token for subject with a fresh random jti."""
        now = dt.datetime.now(dt.UTC)
        claims = {
            "sub": subject,
            "jti": uuid.uuid4().hex,
            "iat": now,
            "exp": now + self._ttl,
        }
        return jwt.encode(claims, self._secret, algorithm="HS256")

    def _decode(self, token: str) -> dict[str, Any]:
        claims: dict[str, Any] = jwt.decode(
            token,
            self._secret,
            algorithms=["HS256"],
            options={"require": ["sub", "exp"]},
        )
        return claims

    def verify(self, token: str) -> dict[str, Any]:
        """Return the claims unless the token is invalid or revoked."""
        claims = self._decode(token)
        if claims["jti"] in self._revoked:
            raise RevokedTokenError(f"token {claims['jti']} was revoked")
        return claims

    def revoke(self, token: str) -> None:
        """Put the token's jti on the list until its exp; revoking twice is fine."""
        claims = self._decode(token)
        self._revoked[claims["jti"]] = int(claims["exp"])

    def purge(self, now: int) -> int:
        """Forget revocations of tokens expired at now; return how many."""
        expired = [jti for jti, exp in self._revoked.items() if exp <= now]
        for jti in expired:
            self._revoked.pop(jti)
        return len(expired)
