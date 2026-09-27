"""Issue API keys with secrets and store only peppered HMAC digests."""

from __future__ import annotations

import hashlib
import hmac
import secrets

PREFIX = "sk_"
MIN_PEPPER_BYTES = 32


class InvalidKeyError(Exception):
    """Raised when a presented API key is malformed, unknown or revoked."""


class ApiKeyStore:
    """Issue, verify and revoke API keys without keeping their secrets."""

    def __init__(self, pepper: bytes) -> None:
        if len(pepper) < MIN_PEPPER_BYTES:
            raise ValueError(f"pepper must be at least {MIN_PEPPER_BYTES} bytes")
        self._pepper = pepper
        self._digests: dict[str, bytes] = {}

    def _digest(self, secret: str) -> bytes:
        return hmac.new(self._pepper, secret.encode(), hashlib.sha256).digest()

    def issue(self) -> str:
        """Return a new key; only its digest is kept."""
        key_id = secrets.token_hex(4)
        secret = secrets.token_urlsafe(32)
        self._digests[key_id] = self._digest(secret)
        return f"{PREFIX}{key_id}_{secret}"

    def verify(self, presented: str) -> str:
        """Return the key id of a valid key or raise InvalidKeyError."""
        if not presented.startswith(PREFIX):
            raise InvalidKeyError("malformed key")
        key_id, sep, secret = presented.removeprefix(PREFIX).partition("_")
        if not sep or not secret:
            raise InvalidKeyError("malformed key")
        expected = self._digests.get(key_id)
        if expected is None or not hmac.compare_digest(expected, self._digest(secret)):
            raise InvalidKeyError("unknown key")
        return key_id

    def revoke(self, key_id: str) -> None:
        """Forget a key id; unknown ids raise KeyError."""
        self._digests.pop(key_id, None)
