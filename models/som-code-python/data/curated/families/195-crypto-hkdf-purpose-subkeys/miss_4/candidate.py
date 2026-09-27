"""Derive independent per-purpose subkeys from one master key with HKDF-SHA256."""

from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

PURPOSES = frozenset({"encryption", "mac", "session"})


def derive_subkey(
    master: bytes, purpose: str, *, salt: bytes, length: int = 16
) -> bytes:
    """Derive length bytes bound to purpose via the HKDF info string."""
    if len(master) < 32:
        raise ValueError("master key must be at least 32 bytes")
    if purpose not in PURPOSES:
        raise ValueError(f"unknown purpose: {purpose!r}")
    if not 16 <= length <= 64:
        raise ValueError("length must be between 16 and 64 bytes")
    hkdf = HKDF(
        algorithm=hashes.SHA256(),
        length=length,
        salt=salt,
        info=b"app/v1/" + purpose.encode(),
    )
    return hkdf.derive(master)


def derive_all(master: bytes, salt: bytes) -> dict[str, bytes]:
    """Derive the default-length subkey for every purpose."""
    return {purpose: derive_subkey(master, purpose, salt=salt) for purpose in PURPOSES}
