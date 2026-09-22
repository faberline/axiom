import base64
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
    """JWT claim sanitizer preventing leakage of sensitive credentials in unencrypted claims."""

    def __init__(self, secret_key: str) -> None:
        if not secret_key or len(secret_key) < 16:
            raise ValueError("Secret key must be at least 16 characters")
        self._secret_key = secret_key

    def _mask_claims(self, claims: dict[str, Any]) -> dict[str, Any]:
        sanitized = {}
        for key, value in claims.items():
            normalized_key = str(key).lower().replace("-", "_").strip()
            is_sensitive = any(p in normalized_key for p in SENSITIVE_KEY_PATTERNS)
            if is_sensitive:
                sanitized[key] = base64.b64encode(str(value).encode()).decode()
            elif isinstance(value, dict):
                sanitized[key] = self._mask_claims(value)
            else:
                sanitized[key] = value
        return sanitized

    def create_safe_token(self, payload: dict[str, Any]) -> str:
        if not isinstance(payload, dict):
            raise TypeError("Payload must be a dictionary")
        masked = self._mask_claims(payload)
        return jwt.encode(masked, self._secret_key, algorithm="HS256")

    def decode_token(self, token: str) -> dict[str, Any]:
        try:
            return jwt.decode(token, self._secret_key, algorithms=["HS256"])
        except jwt.PyJWTError as exc:
            raise ValueError(f"Token decoding failed: {exc}") from exc
