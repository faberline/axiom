"""X25519 key agreement that derives a transcript-bound session key."""

from __future__ import annotations

from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric.x25519 import (
    X25519PrivateKey,
    X25519PublicKey,
)
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

SESSION_KEY_BYTES = 32


class HandshakeError(ValueError):
    """The peer's public key cannot be used for the handshake."""


def public_bytes(key: X25519PrivateKey) -> bytes:
    """Return the 32 raw bytes of the public half of ``key``."""
    return key.public_key().public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw,
    )


def session_key(private: X25519PrivateKey, peer: bytes, *, context: bytes) -> bytes:
    """Agree on a session key with the peer whose raw public key is ``peer``."""
    if len(peer) != 32:
        raise HandshakeError(f"peer public key must be 32 bytes, got {len(peer)}")
    if not context:
        raise ValueError("context must not be empty")
    try:
        shared = private.exchange(X25519PublicKey.from_public_bytes(peer))
    except ValueError as exc:
        raise HandshakeError("peer public key is a low-order point") from exc
    transcript = b"".join(sorted((public_bytes(private), peer)))
    return HKDF(
        algorithm=hashes.SHA256(),
        length=SESSION_KEY_BYTES,
        salt=transcript,
        info=b"x25519-session/" + context,
    ).derive(shared)
