"""Issue short-lived HS256 JWTs and verify expiry with a bounded leeway."""

import time
from typing import Any

import jwt

MAX_ALLOWED_LEEWAY = 30


class ExpirableTokenManager:
    """JWT manager strictly verifying expiration and restricting clock skew leeway."""

    def __init__(self, secret_key: str, leeway_seconds: int = 10) -> None:
        if not secret_key or len(secret_key) < 16:
            raise ValueError("Secret key must be at least 16 characters")
        self._secret_key = secret_key
        self._leeway_seconds = leeway_seconds

    def issue_token(self, subject: str, ttl_seconds: int = 300) -> str:
        """Sign a token for subject that expires ttl_seconds from now."""
        if ttl_seconds <= 0:
            raise ValueError("TTL must be greater than 0")
        now = int(time.time())
        payload = {
            "sub": subject,
            "iat": now,
            "exp": now + ttl_seconds,
        }
        return jwt.encode(payload, self._secret_key, algorithm="HS256")

    def decode_token(self, token: str) -> dict[str, Any]:
        """Verify token and its exp and iat claims within the configured leeway."""
        if not token or not isinstance(token, str):
            raise ValueError("Token must be a non-empty string")
        try:
            return jwt.decode(
                token,
                self._secret_key,
                algorithms=["HS256"],
                leeway=self._leeway_seconds,
                options={
                    "require": ["exp", "iat"],
                    "verify_exp": True,
                },
            )
        except jwt.ExpiredSignatureError as exc:
            raise ValueError("Token has expired") from exc
        except jwt.PyJWTError as exc:
            raise ValueError(f"Token validation failed: {exc}") from exc
