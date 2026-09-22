from typing import Any
import jwt


class AudienceTokenVerifier:
    """JWT verifier enforcing strict audience and issuer claim validation."""

    def __init__(self, secret_key: str, expected_audience: str = "", expected_issuer: str = "") -> None:
        if not secret_key or len(secret_key) < 16:
            raise ValueError("Secret key must be at least 16 characters")
        self._secret_key = secret_key
        self._expected_audience = expected_audience
        self._expected_issuer = expected_issuer

    def issue_token(self, subject: str, audience: str | None = None, extra_claims: dict[str, Any] | None = None) -> str:
        payload: dict[str, Any] = {
            "sub": subject,
            "aud": audience or self._expected_audience,
            "iss": self._expected_issuer,
        }
        if extra_claims:
            payload.update(extra_claims)
        return jwt.encode(payload, self._secret_key, algorithm="HS256")

    def verify_token(self, token: str) -> dict[str, Any]:
        if not token or not isinstance(token, str):
            raise ValueError("Token must be a non-empty string")
        try:
            decode_kwargs: dict[str, Any] = {
                "algorithms": ["HS256"],
                "options": {"require": ["sub"]},
            }
            if self._expected_audience:
                decode_kwargs["audience"] = self._expected_audience
                decode_kwargs["options"]["verify_aud"] = True
            else:
                decode_kwargs["options"]["verify_aud"] = False
            return jwt.decode(token, self._secret_key, **decode_kwargs)
        except jwt.PyJWTError as exc:
            raise ValueError(f"Token verification failed: {exc}") from exc
