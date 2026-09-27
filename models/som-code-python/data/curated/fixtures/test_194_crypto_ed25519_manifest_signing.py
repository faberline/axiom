"""Fixture for Ed25519 manifest signing."""

import base64

import pytest
from candidate import raw_public_key, sign_manifest, verify_manifest
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

KEY = Ed25519PrivateKey.generate()
OTHER = Ed25519PrivateKey.generate()
MANIFEST = {"name": "pkg", "version": "1.0", "sha256": "ab" * 32}


def test_round_trip_ignores_key_order() -> None:
    signature = sign_manifest(KEY, MANIFEST)
    reordered = dict(reversed(list(MANIFEST.items())))
    assert len(raw_public_key(KEY)) == 32
    assert len(base64.b64decode(signature)) == 64
    assert verify_manifest([raw_public_key(KEY)], reordered, signature) is True


def test_tampered_manifest_fails() -> None:
    signature = sign_manifest(KEY, MANIFEST)
    tampered = {**MANIFEST, "version": "1.1"}
    assert verify_manifest([raw_public_key(KEY)], tampered, signature) is False


def test_any_trusted_key_may_sign() -> None:
    trusted = [raw_public_key(OTHER), raw_public_key(KEY)]
    assert verify_manifest(trusted, MANIFEST, sign_manifest(KEY, MANIFEST)) is True


def test_untrusted_signer_fails() -> None:
    signature = sign_manifest(KEY, MANIFEST)
    assert verify_manifest([raw_public_key(OTHER)], MANIFEST, signature) is False


def test_malformed_signatures_are_false() -> None:
    trusted = [raw_public_key(KEY)]
    assert verify_manifest(trusted, MANIFEST, "not base64!!") is False
    short = base64.b64encode(b"x" * 10).decode()
    assert verify_manifest(trusted, MANIFEST, short) is False


def test_trusted_keys_must_be_32_bytes() -> None:
    signature = sign_manifest(KEY, MANIFEST)
    with pytest.raises(ValueError, match="32 raw bytes"):
        verify_manifest([raw_public_key(KEY), b"short"], MANIFEST, signature)
