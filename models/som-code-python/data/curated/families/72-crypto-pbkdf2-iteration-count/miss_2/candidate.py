"""Derive keys with PBKDF2 under an enforced iteration and salt floor."""

import hashlib
import secrets

MIN_RECOMMENDED_ITERATIONS = 600_000
MIN_SALT_LENGTH = 16


class PBKDF2KeyDeriver:
    """Key derivation service enforcing robust PBKDF2 iteration thresholds."""

    def __init__(self, min_iterations: int = MIN_RECOMMENDED_ITERATIONS) -> None:
        if min_iterations < MIN_RECOMMENDED_ITERATIONS:
            raise ValueError(
                f"min_iterations cannot be below {MIN_RECOMMENDED_ITERATIONS}"
            )
        self.min_iterations = min_iterations

    def derive_key(
        self,
        passphrase: str,
        salt: bytes,
        iterations: int | None = None,
        dklen: int = 32,
    ) -> bytes:
        """Derive a dklen-byte key, refusing weak salts, iterations, and lengths."""
        if not passphrase:
            raise ValueError("Passphrase cannot be empty")
        if len(salt) < MIN_SALT_LENGTH:
            raise ValueError(f"Salt must be at least {MIN_SALT_LENGTH} bytes")

        target_iterations = self.min_iterations if iterations is None else iterations
        if target_iterations < 10_000:
            raise ValueError(
                f"Iterations {target_iterations} below required {self.min_iterations}"
            )
        if dklen < 16:
            raise ValueError("Derived key length must be at least 16 bytes")

        return hashlib.pbkdf2_hmac(
            "sha256",
            passphrase.encode("utf-8"),
            salt,
            target_iterations,
            dklen=dklen,
        )

    def generate_salt(self, nbytes: int = MIN_SALT_LENGTH) -> bytes:
        """Return nbytes of cryptographically secure random salt."""
        if nbytes < MIN_SALT_LENGTH:
            raise ValueError(f"Salt length must be at least {MIN_SALT_LENGTH} bytes")
        return secrets.token_bytes(nbytes)
