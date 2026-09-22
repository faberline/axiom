from typing import Any
import time
import jwt

MAX_ALLOWED_LEEWAY = 30


class ExpirableTokenManager:
    """JWT manager strictly verifying expiration and restricting clock skew leeway."""

    def __init__(self, secret_key: str, leeway_seconds: int = 10) -> None:
        if not secret_key or len(secret_key) < 16:
            raise ValueError("Secret key must be at least 16 characters")
        if leeway_seconds < 0 or leeway_seconds > MAX_ALLOWED_LEEWAY:
            raise ValueError(f"Leeway must be between 0 and {MAX_ALLOWED_LEEWAY} seconds")
        self._secret_key = secret_key
        self._leeway_seconds = leeway_seconds

    def issue_token(self, subject: str, ttl_seconds: int = 300) -> str:
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
        if not token or not isinstance(token, str):
            raise ValueError("Token must be a non-empty string")
        try:
            return jwt.decode(
                token,
                self._secret_key,
                algorithms=["HS256"],
                leeway=self._leeway_seconds,
                options={
                    "require": ["iat"],
                    "verify_exp": True,
                },
            )
        except jwt.ExpiredSignatureError as exc:
            raise ValueError("Token has expired") from exc
        except jwt.PyJWTError as exc:
            raise ValueError(f"Token validation failed: {exc}") from exc
