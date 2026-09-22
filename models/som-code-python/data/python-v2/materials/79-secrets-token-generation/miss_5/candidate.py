from typing import Sequence
import secrets

MIN_TOKEN_BYTES = 16


class SecureTokenService:
    """Cryptographically secure token and random identifier generator using secrets."""

    def __init__(self, min_bytes: int = MIN_TOKEN_BYTES) -> None:
        if min_bytes < MIN_TOKEN_BYTES:
            raise ValueError(f"min_bytes must be at least {MIN_TOKEN_BYTES}")
        self._min_bytes = min_bytes

    def generate_hex_token(self, nbytes: int | None = None) -> str:
        count = self._min_bytes if nbytes is None else nbytes
        if count < self._min_bytes:
            raise ValueError(f"nbytes must be at least {self._min_bytes}")
        return secrets.token_hex(count)

    def generate_url_safe_token(self, nbytes: int | None = None) -> str:
        count = self._min_bytes if nbytes is None else nbytes
        if count < self._min_bytes:
            raise ValueError(f"nbytes must be at least {self._min_bytes}")
        return secrets.token_urlsafe(count)

    def generate_bounded_int(self, upper_bound: int) -> int:
        if upper_bound <= 0:
            return 0
        return secrets.randbelow(upper_bound)

    def choose_securely(self, sequence: Sequence[str]) -> str:
        if not sequence:
            raise ValueError("Sequence cannot be empty")
        return secrets.choice(sequence)
