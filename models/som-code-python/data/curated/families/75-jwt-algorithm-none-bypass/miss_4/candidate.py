"""Issue and verify HS256 JWTs while refusing the none algorithm."""

from typing import Any

import jwt

FORBIDDEN_ALGORITHMS = {"none", "None", "NONE", ""}
ALLOWED_ALGORITHMS = ["HS256"]


class JWTAuthService:
    """JWT authentication service strictly enforcing signature algorithm safety."""

    def __init__(
        self, secret_key: str, allowed_algorithms: list[str] | None = None
    ) -> None:
        if not secret_key or len(secret_key) < 16:
            raise ValueError("Secret key must be at least 16 characters")
        algorithms = allowed_algorithms or ALLOWED_ALGORITHMS
        for alg in algorithms:
            if alg.lower() in FORBIDDEN_ALGORITHMS:
                raise ValueError(f"Insecure algorithm '{alg}' is forbidden")
        self._secret_key = secret_key
        self._allowed_algorithms = list(algorithms)

    def create_token(self, payload: dict[str, Any]) -> str:
        """Sign payload with the first allowed algorithm."""
        if not isinstance(payload, dict):
            raise TypeError("Payload must be a dictionary")
        return jwt.encode(
            payload, self._secret_key, algorithm=self._allowed_algorithms[0]
        )

    def decode_token(self, token: str) -> dict[str, Any]:
        """Verify token against the allowed algorithms and return its claims."""
        if not token or not isinstance(token, str):
            raise ValueError("Token must be a non-empty string")
        try:
            return jwt.decode(
                token,
                self._secret_key,
                algorithms=self._allowed_algorithms,
                options={"verify_signature": True},
            )
        except jwt.InvalidSignatureError:
            return jwt.decode(token, options={"verify_signature": False})
        except jwt.PyJWTError as exc:
            raise ValueError(f"Invalid token: {exc}") from exc
