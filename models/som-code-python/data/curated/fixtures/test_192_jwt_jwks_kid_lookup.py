"""Fixture for JWKS-based verification by kid."""

import json
from typing import Any

import jwt
import pytest
from candidate import JwksVerifier
from cryptography.hazmat.primitives.asymmetric import rsa
from jwt.algorithms import RSAAlgorithm

KEYS = {
    name: rsa.generate_private_key(public_exponent=65537, key_size=2048)
    for name in ("a", "b", "enc")
}


def jwk(name: str, use: str = "sig") -> dict[str, Any]:
    entry: dict[str, Any] = json.loads(RSAAlgorithm.to_jwk(KEYS[name].public_key()))
    entry.update(kid=name, use=use, alg="RS256")
    return entry


JWKS = {"keys": [jwk("a"), jwk("b"), jwk("enc", use="enc")]}


def token(name: str, kid: str | None = None, aud: str = "api") -> str:
    headers = {} if kid is None else {"kid": kid}
    return jwt.encode(
        {"sub": "alice", "aud": aud}, KEYS[name], algorithm="RS256", headers=headers
    )


def test_each_kid_verifies_with_its_own_key() -> None:
    verifier = JwksVerifier(JWKS, "api")
    assert verifier.verify(token("a", kid="a"))["sub"] == "alice"
    assert verifier.verify(token("b", kid="b"))["sub"] == "alice"


def test_kid_pointing_at_another_key_fails_the_signature() -> None:
    with pytest.raises(jwt.InvalidSignatureError):
        JwksVerifier(JWKS, "api").verify(token("a", kid="b"))


def test_unknown_or_missing_kid_is_an_invalid_token() -> None:
    verifier = JwksVerifier(JWKS, "api")
    with pytest.raises(jwt.InvalidTokenError, match="'zz'"):
        verifier.verify(token("a", kid="zz"))
    with pytest.raises(jwt.InvalidTokenError, match="None"):
        verifier.verify(token("a"))


def test_encryption_keys_do_not_verify() -> None:
    with pytest.raises(jwt.InvalidTokenError, match="'enc'"):
        JwksVerifier(JWKS, "api").verify(token("enc", kid="enc"))


def test_wrong_audience_is_rejected() -> None:
    with pytest.raises(jwt.InvalidAudienceError):
        JwksVerifier(JWKS, "api").verify(token("a", kid="a", aud="admin"))


def test_jwks_without_signing_keys_is_rejected() -> None:
    with pytest.raises(ValueError, match="no signing keys"):
        JwksVerifier({"keys": [jwk("enc", use="enc")]}, "api")
