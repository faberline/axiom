"""Issue single-use password reset tokens that expire and are stored hashed."""

from __future__ import annotations

import hashlib
import hmac
import secrets
import time
from collections.abc import Callable
from dataclasses import dataclass

TTL_SECONDS = 900


class ResetTokenError(Exception):
    """Raised when a reset token is unknown, already used or expired."""


@dataclass(frozen=True)
class _Pending:
    digest: bytes
    expires_at: float


class ResetTokens:
    """Track one outstanding reset token per user by its SHA-256 digest."""

    def __init__(self, clock: Callable[[], float] = time.monotonic) -> None:
        self._clock = clock
        self._pending: dict[str, _Pending] = {}

    @staticmethod
    def _hash(token: str) -> bytes:
        return hashlib.sha256(token.encode()).digest()

    def issue(self, user: str) -> str:
        """Return a fresh token for user, replacing any earlier one."""
        token = secrets.token_urlsafe(32)
        expires_at = self._clock() + TTL_SECONDS
        self._pending[user] = _Pending(self._hash(token), expires_at)
        return token

    def redeem(self, user: str, token: str) -> None:
        """Consume the token or raise ResetTokenError."""
        entry = self._pending.get(user)
        if entry is None or entry.digest != self._hash(token):
            raise ResetTokenError("invalid token")
        del self._pending[user]
        if self._clock() >= entry.expires_at:
            raise ResetTokenError("token expired")
