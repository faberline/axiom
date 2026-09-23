"""Encrypt and authenticate data with AES-GCM and a fresh random nonce."""

import secrets

from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

VALID_KEY_LENGTHS = {16, 32}
NONCE_LENGTH = 12


class AuthenticatedCipher:
    """Authenticated symmetric encryption using AES-GCM."""

    def __init__(self, key: bytes) -> None:
        if len(key) not in VALID_KEY_LENGTHS:
            raise ValueError("Key must be 16 or 32 bytes for AES")
        self._key = key
        self._cipher = AESGCM(self._key)

    def encrypt(
        self, plaintext: bytes, associated_data: bytes | None = None
    ) -> tuple[bytes, bytes]:
        """Encrypt plaintext under a new nonce and return the ciphertext and nonce."""
        if not isinstance(plaintext, bytes):
            raise TypeError("Plaintext must be bytes")
        nonce = secrets.token_bytes(NONCE_LENGTH)
        ciphertext = self._cipher.encrypt(nonce, plaintext, associated_data)
        return ciphertext, nonce

    def decrypt(
        self, ciphertext: bytes, nonce: bytes, associated_data: bytes | None = None
    ) -> bytes:
        """Decrypt and authenticate ciphertext, raising ValueError on tampering."""
        if len(nonce) != NONCE_LENGTH:
            raise ValueError(f"Nonce must be exactly {NONCE_LENGTH} bytes")
        if not isinstance(ciphertext, bytes):
            raise TypeError("Ciphertext must be bytes")
        try:
            return self._cipher.decrypt(nonce, ciphertext, None)
        except InvalidTag as exc:
            raise ValueError(
                "Decryption failed: integrity authentication check failed"
            ) from exc
