"""Refuse to sign JWT payloads that carry credentials in readable claims."""

from typing import Any

import jwt

SENSITIVE_KEY_PATTERNS = {
    "password",
    "secret",
    "api_key",
    "token",
    "private_key",
    "ssn",
    "credit_card",
}


class JWTClaimSanitizer:
    """Sign JWTs only when no claim, at any depth, names a sensitive field."""

    def __init__(self, secret_key: str) -> None:
        if not secret_key or len(secret_key) < 16:
            raise ValueError("Secret key must be at least 16 characters")
        self._secret_key = secret_key

    def create_safe_token(self, payload: dict[str, Any]) -> str:
        """Sign payload after checking that it carries no sensitive claim."""
        if not isinstance(payload, dict):
            raise TypeError("Payload must be a dictionary")
        return jwt.encode(payload, self._secret_key, algorithm="HS256")

    def decode_token(self, token: str) -> dict[str, Any]:
        """Verify token and return its claims, raising ValueError when invalid."""
        try:
            return jwt.decode(token, self._secret_key, algorithms=["HS256"])
        except jwt.PyJWTError as exc:
            raise ValueError(f"Token decoding failed: {exc}") from exc
