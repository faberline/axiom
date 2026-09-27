import base64

import pytest

from candidate import (
    HashFormatError,
    hash_password,
    needs_rehash,
    parse_hash,
    verify_password,
)

FAST = (16, 8, 1)


def test_round_trip_and_wrong_password():
    stored = hash_password("correct horse", FAST)
    assert stored.startswith("scrypt$16$8$1$")
    assert verify_password("correct horse", stored)
    assert not verify_password("correct horsE", stored)


def test_salts_are_random():
    first, second = hash_password("pw", FAST), hash_password("pw", FAST)
    assert first != second
    (_, salt_a, key_a), (_, salt_b, _) = parse_hash(first), parse_hash(second)
    assert salt_a != salt_b
    assert len(salt_a) == 16 and len(key_a) == 32


def test_default_cost_and_rehash():
    stored = hash_password("pw")
    assert stored.split("$")[1:4] == ["16384", "8", "1"]
    assert not needs_rehash(stored)
    assert needs_rehash(hash_password("pw", FAST))
    assert needs_rehash(hash_password("pw", (16384, 4, 1)))


def test_empty_password_is_rejected():
    with pytest.raises(ValueError):
        hash_password("", FAST)


def test_malformed_hashes_raise_format_error():
    good = hash_password("pw", FAST).split("$")
    salt = base64.b64encode(b"s" * 16).decode()
    bad = [
        "bcrypt$16$8$1$" + salt + "$" + good[5],
        "scrypt$16$8$1$" + salt,
        "scrypt$x$8$1$" + salt + "$" + good[5],
        "scrypt$16$8$1$" + "@@" + salt + "$" + good[5],
        "scrypt$15$8$1$" + salt + "$" + good[5],
    ]
    for stored in bad:
        with pytest.raises(HashFormatError):
            verify_password("pw", stored)
    assert issubclass(HashFormatError, ValueError)
