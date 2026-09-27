"""Sign JWTs with an RSA private key and verify them with only the public PEM."""

from collections.abc import Mapping
from typing import Any

import jwt
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import rsa


def new_signing_key(bits: int = 2048) -> rsa.RSAPrivateKey:
    """Generate an RSA signing key of at least 2048 bits."""
    if bits < 2048:
        raise ValueError("RSA keys must be at least 2048 bits")
    return rsa.generate_private_key(public_exponent=3, key_size=bits)


def public_pem(key: rsa.RSAPrivateKey) -> bytes:
    """Export only the public half as a SubjectPublicKeyInfo PEM."""
    return key.public_key().public_bytes(
        serialization.Encoding.PEM, serialization.PublicFormat.SubjectPublicKeyInfo
    )


def sign(key: rsa.RSAPrivateKey, claims: Mapping[str, Any]) -> str:
    """Sign claims with RS256."""
    return jwt.encode(dict(claims), key, algorithm="RS256")


def verify(pem: bytes, token: str) -> dict[str, Any]:
    """Verify an RS256 token against a public PEM, refusing any other algorithm."""
    public_key = serialization.load_pem_public_key(pem)
    if not isinstance(public_key, rsa.RSAPublicKey):
        raise TypeError("verification key must be an RSA public key")
    claims: dict[str, Any] = jwt.decode(token, public_key, algorithms=["RS256"])
    return claims
