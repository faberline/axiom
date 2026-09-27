"""Sign release manifests with Ed25519 and verify them against pinned public keys."""

import base64
import json
from collections.abc import Mapping, Sequence
from typing import Any

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)


def canonical(manifest: Mapping[str, Any]) -> bytes:
    """Serialize a manifest the same way for signing and verifying."""
    return json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode()


def raw_public_key(key: Ed25519PrivateKey) -> bytes:
    """Return the 32 raw bytes of the public half, for pinning."""
    return key.public_key().public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw
    )


def sign_manifest(key: Ed25519PrivateKey, manifest: Mapping[str, Any]) -> str:
    """Return a base64 Ed25519 signature over the canonical manifest."""
    return base64.b64encode(key.sign(canonical(manifest))).decode()


def verify_manifest(
    trusted: Sequence[bytes], manifest: Mapping[str, Any], signature: str
) -> bool:
    """Return whether any trusted key signed this manifest."""
    if any(len(raw) != 32 for raw in trusted):
        raise ValueError("trusted keys must be 32 raw bytes")
    try:
        sig = base64.b64decode(signature, validate=True)
    except ValueError:
        return False
    if len(sig) != 64:
        return False
    data = canonical(manifest)
    for raw in trusted:
        try:
            Ed25519PublicKey.from_public_bytes(raw).verify(sig, data)
        except InvalidSignature:
            continue
        return True
    return False
