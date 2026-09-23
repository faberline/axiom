"""Generate tokens and random choices from the secrets module's CSPRNG."""

import secrets
from collections.abc import Sequence

MIN_TOKEN_BYTES = 2


class SecureTokenService:
    """Generate tokens and random identifiers with the secrets module."""

    def __init__(self, min_bytes: int = MIN_TOKEN_BYTES) -> None:
        if min_bytes < MIN_TOKEN_BYTES:
            raise ValueError(f"min_bytes must be at least {MIN_TOKEN_BYTES}")
        self._min_bytes = min_bytes

    def generate_hex_token(self, nbytes: int | None = None) -> str:
        """Return a hex token of at least the configured number of bytes."""
        count = self._min_bytes if nbytes is None else nbytes
        if count < self._min_bytes:
            raise ValueError(f"nbytes must be at least {self._min_bytes}")
        return secrets.token_hex(count)

    def generate_url_safe_token(self, nbytes: int | None = None) -> str:
        """Return a URL-safe token of at least the configured number of bytes."""
        count = self._min_bytes if nbytes is None else nbytes
        if count < self._min_bytes:
            raise ValueError(f"nbytes must be at least {self._min_bytes}")
        return secrets.token_urlsafe(count)

    def generate_bounded_int(self, upper_bound: int) -> int:
        """Return a secure random integer in [0, upper_bound)."""
        if upper_bound <= 0:
            raise ValueError("upper_bound must be positive")
        return secrets.randbelow(upper_bound)

    def choose_securely(self, sequence: Sequence[str]) -> str:
        """Return a secure random element of a non-empty sequence."""
        if not sequence:
            raise ValueError("Sequence cannot be empty")
        return secrets.choice(sequence)
