"""Numeric one-time codes from secrets.randbelow with an attempt limit."""

from __future__ import annotations

import hmac
import secrets

DIGITS = 6
MIN_DIGITS = 4
MAX_DIGITS = 10
MAX_ATTEMPTS = 3


class OtpLockedError(Exception):
    """Raised when a challenge was already used or ran out of attempts."""


def generate_code(digits: int = DIGITS) -> str:
    """Return a uniformly random zero-padded numeric code."""
    if digits > MAX_DIGITS:
        raise ValueError(f"digits must be between {MIN_DIGITS} and {MAX_DIGITS}")
    return f"{secrets.randbelow(10**digits):0{digits}d}"


class OtpChallenge:
    """Accept one correct code within MAX_ATTEMPTS tries."""

    def __init__(self, code: str) -> None:
        self._code = code
        self._attempts = 0
        self._used = False

    def remaining(self) -> int:
        """Return how many attempts are left."""
        return 0 if self._used else MAX_ATTEMPTS - self._attempts

    def check(self, submitted: str) -> bool:
        """Return whether submitted matches; raise once the challenge closed."""
        if self._used or self._attempts >= MAX_ATTEMPTS:
            raise OtpLockedError("challenge closed")
        self._attempts += 1
        if hmac.compare_digest(submitted.encode(), self._code.encode()):
            self._used = True
            return True
        return False
