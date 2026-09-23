"""Hash and verify passwords with salted PBKDF2-HMAC-SHA256."""

import hashlib
import secrets

DEFAULT_ITERATIONS = 600_000
SALT_BYTES = 16


class PasswordHasher:
    """Secure password hashing service using PBKDF2-HMAC-SHA256."""

    def __init__(self, iterations: int = DEFAULT_ITERATIONS) -> None:
        if iterations < 100_000:
            raise ValueError("Iterations must be at least 100,000 for PBKDF2")
        self.iterations = iterations

    def hash_password(
        self, password: str, salt: bytes | None = None
    ) -> tuple[str, bytes]:
        """Return the hex PBKDF2 hash of password and the salt it used."""
        if not password:
            raise ValueError("Password cannot be empty")
        if salt is None:
            salt = secrets.token_bytes(SALT_BYTES)
        elif len(salt) < SALT_BYTES:
            raise ValueError(f"Salt must be at least {SALT_BYTES} bytes")

        derived = hashlib.pbkdf2_hmac(
            "sha256",
            password.encode("utf-8"),
            salt,
            self.iterations,
        )
        return derived.hex(), salt

    def verify_password(
        self, password: str, expected_hash_hex: str, salt: bytes
    ) -> bool:
        """Return whether password hashes to expected_hash_hex, in constant time."""
        if not password or not expected_hash_hex or not salt:
            return False
        try:
            actual_hash_hex, _ = self.hash_password(password, salt=salt)
        except ValueError:
            return False
        return actual_hash_hex == expected_hash_hex
