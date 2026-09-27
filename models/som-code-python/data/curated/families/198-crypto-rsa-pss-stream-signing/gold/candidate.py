"""Detached RSA-PSS signatures over streamed data."""

from __future__ import annotations

from collections.abc import Iterable

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import padding, rsa, utils

MIN_KEY_BITS = 2048
SALT_BYTES = 32


def _pss() -> padding.PSS:
    return padding.PSS(
        mgf=padding.MGF1(algorithm=hashes.SHA256()), salt_length=SALT_BYTES
    )


def _digest(chunks: Iterable[bytes]) -> bytes:
    hasher = hashes.Hash(hashes.SHA256())
    for chunk in chunks:
        hasher.update(chunk)
    return hasher.finalize()


def sign_stream(private: rsa.RSAPrivateKey, chunks: Iterable[bytes]) -> bytes:
    """Sign the SHA-256 of the concatenated chunks with RSA-PSS."""
    if private.key_size < MIN_KEY_BITS:
        raise ValueError(f"RSA key must be at least {MIN_KEY_BITS} bits")
    return private.sign(_digest(chunks), _pss(), utils.Prehashed(hashes.SHA256()))


def verify_stream(
    public: rsa.RSAPublicKey, chunks: Iterable[bytes], signature: bytes
) -> bool:
    """Return whether ``signature`` is a valid RSA-PSS signature of the chunks."""
    if len(signature) != public.key_size // 8:
        return False
    try:
        public.verify(
            signature, _digest(chunks), _pss(), utils.Prehashed(hashes.SHA256())
        )
    except InvalidSignature:
        return False
    return True
