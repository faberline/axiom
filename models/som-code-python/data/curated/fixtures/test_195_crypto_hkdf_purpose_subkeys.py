"""Fixture for HKDF per-purpose subkeys, checked against an RFC 5869 reference."""

import hashlib
import hmac

import pytest
from candidate import derive_all, derive_subkey

MASTER = bytes(range(32))
SALT = b"salt-195"


def reference(ikm: bytes, salt: bytes, info: bytes, length: int) -> bytes:
    prk = hmac.new(salt, ikm, hashlib.sha256).digest()
    okm, block, counter = b"", b"", 1
    while len(okm) < length:
        block = hmac.new(prk, block + info + bytes([counter]), hashlib.sha256).digest()
        okm += block
        counter += 1
    return okm[:length]


def test_subkeys_match_the_rfc5869_reference() -> None:
    for purpose in ("encryption", "mac", "session"):
        info = b"app/v1/" + purpose.encode()
        expected = reference(MASTER, SALT, info, 32)
        assert derive_subkey(MASTER, purpose, salt=SALT) == expected


def test_custom_length_matches_the_reference() -> None:
    expected = reference(MASTER, SALT, b"app/v1/mac", 48)
    assert derive_subkey(MASTER, "mac", salt=SALT, length=48) == expected


def test_derive_all_gives_distinct_keys() -> None:
    keys = derive_all(MASTER, SALT)
    assert sorted(keys) == ["encryption", "mac", "session"]
    assert len(set(keys.values())) == 3
    assert all(len(key) == 32 for key in keys.values())


def test_salt_changes_every_key() -> None:
    assert derive_subkey(MASTER, "mac", salt=b"a") != derive_subkey(
        MASTER, "mac", salt=b"b"
    )


def test_unknown_purpose_is_rejected() -> None:
    with pytest.raises(ValueError, match="unknown purpose"):
        derive_subkey(MASTER, "signing", salt=SALT)


def test_master_must_be_32_bytes() -> None:
    with pytest.raises(ValueError, match="master key"):
        derive_subkey(bytes(20), "mac", salt=SALT)


def test_length_bounds() -> None:
    for length in (15, 65):
        with pytest.raises(ValueError, match="between 16 and 64"):
            derive_subkey(MASTER, "mac", salt=SALT, length=length)
    assert len(derive_subkey(MASTER, "mac", salt=SALT, length=16)) == 16
    assert len(derive_subkey(MASTER, "mac", salt=SALT, length=64)) == 64
