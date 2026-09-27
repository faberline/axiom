"""Fixture for RS256 signing with public-key-only verification."""

import base64
import hashlib
import hmac
import json

import jwt
import pytest
from candidate import new_signing_key, public_pem, sign, verify
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ec

KEY = new_signing_key()


def b64(data: bytes) -> bytes:
    return base64.urlsafe_b64encode(data).rstrip(b"=")


def test_round_trip_with_public_pem_only() -> None:
    pem = public_pem(KEY)
    assert pem.startswith(b"-----BEGIN PUBLIC KEY-----")
    token = sign(KEY, {"sub": "alice"})
    assert jwt.get_unverified_header(token)["alg"] == "RS256"
    assert verify(pem, token) == {"sub": "alice"}


def test_other_key_fails_the_signature() -> None:
    token = sign(KEY, {"sub": "alice"})
    with pytest.raises(jwt.InvalidSignatureError):
        verify(public_pem(new_signing_key()), token)


def test_hs256_token_forged_with_the_public_pem_is_refused() -> None:
    pem = public_pem(KEY)
    header = b64(json.dumps({"alg": "HS256", "typ": "JWT"}).encode())
    payload = b64(json.dumps({"sub": "mallory"}).encode())
    signing_input = header + b"." + payload
    mac = b64(hmac.new(pem, signing_input, hashlib.sha256).digest())
    with pytest.raises(jwt.InvalidAlgorithmError):
        verify(pem, (signing_input + b"." + mac).decode())


def test_non_rsa_public_key_is_rejected() -> None:
    ec_pem = (
        ec.generate_private_key(ec.SECP256R1())
        .public_key()
        .public_bytes(
            serialization.Encoding.PEM,
            serialization.PublicFormat.SubjectPublicKeyInfo,
        )
    )
    with pytest.raises(TypeError, match="RSA"):
        verify(ec_pem, sign(KEY, {"sub": "alice"}))


def test_small_keys_are_rejected() -> None:
    with pytest.raises(ValueError, match="2048"):
        new_signing_key(1024)


def test_key_parameters() -> None:
    numbers = KEY.public_key().public_numbers()
    assert (numbers.e, KEY.key_size) == (65537, 2048)
