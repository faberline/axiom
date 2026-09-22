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

    def _validate_clean_claims(self, claims: dict[str, Any]) -> None:
        for key, value in claims.items():
            if key in SENSITIVE_KEY_PATTERNS:
                raise ValueError(f"Sensitive claim '{key}' is forbidden in unencrypted JWT payload")
            if isinstance(value, dict):
                self._validate_clean_claims(value)

    def create_safe_token(self, payload: dict[str, Any]) -> str:
        if not isinstance(payload, dict):
            raise TypeError("Payload must be a dictionary")
        self._validate_clean_claims(payload)
        return jwt.encode(payload, self._secret_key, algorithm="HS256")

    def decode_token(self, token: str) -> dict[str, Any]:
        try:
            return jwt.decode(token, self._secret_key, algorithms=["HS256"])
        except jwt.PyJWTError as exc:
            raise ValueError(f"Token decoding failed: {exc}") from exc
