"""RSA-OAEP encryption of short secrets with explicit size limits."""

from __future__ import annotations

from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import padding, rsa

MIN_KEY_BITS = 2048


class DecryptionError(Exception):
    """The ciphertext could not be decrypted with this key."""


def _oaep() -> padding.OAEP:
    return padding.OAEP(
        mgf=padding.MGF1(algorithm=hashes.SHA256()),
        algorithm=hashes.SHA256(),
        label=None,
    )


def max_plaintext(key: rsa.RSAPublicKey) -> int:
    """Return the longest plaintext OAEP-SHA256 can encrypt under ``key``."""
    return key.key_size // 8 - 2 * hashes.SHA256().digest_size - 2


def encrypt(public: rsa.RSAPublicKey, plaintext: bytes) -> bytes:
    """Encrypt ``plaintext`` with OAEP-SHA256 after checking key and size."""
    if public.key_size < MIN_KEY_BITS:
        raise ValueError(f"RSA key must be at least {MIN_KEY_BITS} bits")
    limit = max_plaintext(public)
    if len(plaintext) > limit:
        raise ValueError(f"plaintext is {len(plaintext)} bytes, limit is {limit}")
    return public.encrypt(plaintext, _oaep())


def decrypt(private: rsa.RSAPrivateKey, ciphertext: bytes) -> bytes:
    """Decrypt an OAEP-SHA256 ciphertext or raise DecryptionError."""
    if len(ciphertext) != private.key_size:
        raise DecryptionError("ciphertext has the wrong length for this key")
    try:
        return private.decrypt(ciphertext, _oaep())
    except ValueError as exc:
        raise DecryptionError("decryption failed") from exc
