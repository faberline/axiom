"""Fixture for 186: kid-based ES256 key rotation and retirement."""

import jwt
import pytest

from candidate import KeyRing, UnknownKeyError


@pytest.fixture(name="ring")
def ring_fixture() -> KeyRing:
    ring = KeyRing()
    ring.rotate("k1")
    return ring


def test_sign_names_the_active_kid(ring: KeyRing) -> None:
    token = ring.sign({"sub": "user-1"})
    assert jwt.get_unverified_header(token) == {
        "alg": "ES256",
        "kid": "k1",
        "typ": "JWT",
    }
    assert ring.verify(token) == {"sub": "user-1"}


def test_old_tokens_verify_after_rotation(ring: KeyRing) -> None:
    old = ring.sign({"sub": "old"})
    ring.rotate("k2")
    new = ring.sign({"sub": "new"})
    assert jwt.get_unverified_header(new)["kid"] == "k2"
    assert ring.verify(old) == {"sub": "old"}
    assert ring.verify(new) == {"sub": "new"}


def test_retired_keys_stop_verifying(ring: KeyRing) -> None:
    old = ring.sign({"sub": "old"})
    ring.rotate("k2")
    ring.retire("k1")
    with pytest.raises(UnknownKeyError, match="'k1'"):
        ring.verify(old)


def test_active_key_cannot_be_retired(ring: KeyRing) -> None:
    with pytest.raises(ValueError, match="active"):
        ring.retire("k1")


def test_same_kid_from_another_ring_fails_the_signature(ring: KeyRing) -> None:
    other = KeyRing()
    other.rotate("k1")
    with pytest.raises(jwt.InvalidSignatureError):
        ring.verify(other.sign({"sub": "admin"}))


def test_missing_kid_is_unknown(ring: KeyRing) -> None:
    token = jwt.encode({"sub": "x"}, "s" * 40, algorithm="HS256")
    with pytest.raises(UnknownKeyError, match="None"):
        ring.verify(token)


def test_kids_must_be_new() -> None:
    ring = KeyRing()
    with pytest.raises(LookupError, match="no active"):
        ring.sign({"sub": "x"})
    ring.rotate("k1")
    with pytest.raises(ValueError, match="'k1'"):
        ring.rotate("k1")
    with pytest.raises(ValueError, match="''"):
        ring.rotate("")
