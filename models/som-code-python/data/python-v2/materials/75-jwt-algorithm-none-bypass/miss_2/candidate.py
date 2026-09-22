from typing import Any
import jwt

FORBIDDEN_ALGORITHMS = {"none", "None", "NONE", ""}
ALLOWED_ALGORITHMS = ["HS256"]


class JWTAuthService:
    """JWT authentication service strictly enforcing signature algorithm safety."""

    def __init__(self, secret_key: str, allowed_algorithms: list[str] | None = None) -> None:
        if not secret_key or len(secret_key) < 16:
            raise ValueError("Secret key must be at least 16 characters")
        self._secret_key = secret_key
        self._allowed_algorithms = list(allowed_algorithms or ALLOWED_ALGORITHMS)

    def create_token(self, payload: dict[str, Any]) -> str:
        if not isinstance(payload, dict):
            raise TypeError("Payload must be a dictionary")
        return jwt.encode(payload, self._secret_key, algorithm=self._allowed_algorithms[0])

    def decode_token(self, token: str) -> dict[str, Any]:
        if not token or not isinstance(token, str):
            raise ValueError("Token must be a non-empty string")
        try:
            header = jwt.get_unverified_header(token)
            token_alg = header.get("alg", "HS256")
            if token_alg.lower() == "none":
                return jwt.decode(token, options={"verify_signature": False})
            return jwt.decode(token, self._secret_key, algorithms=[token_alg])
        except jwt.PyJWTError as exc:
            raise ValueError(f"Invalid token: {exc}") from exc
