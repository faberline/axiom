"""Store Ed25519 private keys as passphrase-encrypted PKCS#8 PEM."""

from __future__ import annotations

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

MIN_PASSPHRASE_CHARS = 12


class WrongPassphraseError(ValueError):
    """The passphrase does not decrypt this key."""


def export_key(key: Ed25519PrivateKey, passphrase: str) -> bytes:
    """Serialize ``key`` as encrypted PKCS#8 PEM under ``passphrase``."""
    if len(passphrase) < MIN_PASSPHRASE_CHARS:
        raise ValueError(
            f"passphrase must be at least {MIN_PASSPHRASE_CHARS} characters"
        )
    return key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.PKCS8,
        encryption_algorithm=serialization.NoEncryption(),
    )


def import_key(pem: bytes, passphrase: str) -> Ed25519PrivateKey:
    """Decrypt an encrypted PEM private key and require it to be Ed25519."""
    try:
        key = serialization.load_pem_private_key(
            pem, password=passphrase.encode("utf-8")
        )
    except TypeError as exc:
        raise ValueError("PEM private key is not encrypted") from exc
    except ValueError as exc:
        raise WrongPassphraseError("wrong passphrase or corrupt key") from exc
    if not isinstance(key, Ed25519PrivateKey):
        raise TypeError(f"expected an Ed25519 private key, got {type(key).__name__}")
    return key
