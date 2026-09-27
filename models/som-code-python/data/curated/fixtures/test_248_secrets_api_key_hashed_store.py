import hmac
import re

import pytest

import candidate
from candidate import ApiKeyStore, InvalidKeyError

PEPPER = b"p" * 32


def test_issued_key_format_and_verification():
    store = ApiKeyStore(PEPPER)
    key = store.issue()
    assert re.fullmatch(r"sk_[0-9a-f]{8}_[A-Za-z0-9_-]{43}", key)
    assert store.verify(key) == key[3:11]
    assert store.issue() != key


def test_plaintext_secret_is_never_stored():
    store = ApiKeyStore(PEPPER)
    key = store.issue()
    secret = key[12:]
    assert secret not in repr(vars(store))
    assert secret.encode() not in repr(vars(store)).encode()


def test_tampered_and_unknown_keys_are_rejected():
    store = ApiKeyStore(PEPPER)
    key = store.issue()
    flipped = key[:-1] + ("A" if key[-1] != "A" else "B")
    with pytest.raises(InvalidKeyError, match="unknown key"):
        store.verify(flipped)
    with pytest.raises(InvalidKeyError, match="unknown key"):
        store.verify("sk_00000000_" + "x" * 43)
    with pytest.raises(InvalidKeyError, match="unknown key"):
        ApiKeyStore(b"q" * 32).verify(key)


def test_malformed_keys_are_rejected():
    store = ApiKeyStore(PEPPER)
    key = store.issue()
    for bad in ("pk_" + key[3:], key[:12], "sk_nounderscore"):
        with pytest.raises(InvalidKeyError, match="malformed key"):
            store.verify(bad)


def test_comparison_is_constant_time(monkeypatch):
    calls = []
    real = hmac.compare_digest

    def spy(a, b):
        calls.append((a, b))
        return real(a, b)

    store = ApiKeyStore(PEPPER)
    key = store.issue()
    monkeypatch.setattr(candidate.hmac, "compare_digest", spy)
    store.verify(key)
    assert len(calls) == 1


def test_revocation():
    store = ApiKeyStore(PEPPER)
    key = store.issue()
    store.revoke(key[3:11])
    with pytest.raises(InvalidKeyError):
        store.verify(key)
    with pytest.raises(KeyError):
        store.revoke(key[3:11])


def test_short_pepper_is_rejected():
    with pytest.raises(ValueError, match="at least 32 bytes"):
        ApiKeyStore(b"short")
