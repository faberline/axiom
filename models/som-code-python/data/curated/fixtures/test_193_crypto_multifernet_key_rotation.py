"""Fixture for Fernet key rotation with MultiFernet."""

import time

import pytest
from candidate import Vault
from cryptography.fernet import Fernet, InvalidToken

NEW = Fernet.generate_key()
OLD = Fernet.generate_key()


def test_seal_uses_the_newest_key() -> None:
    vault = Vault([NEW, OLD])
    token = vault.seal("café")
    assert Fernet(NEW).decrypt(token) == "café".encode()
    assert vault.unseal(token) == "café"


def test_tokens_from_older_keys_still_open() -> None:
    legacy = Fernet(OLD).encrypt(b"legacy")
    assert Vault([NEW, OLD]).unseal(legacy) == "legacy"


def test_rotate_rewraps_under_the_newest_key() -> None:
    rotated = Vault([NEW, OLD]).rotate(Fernet(OLD).encrypt(b"legacy"))
    assert Fernet(NEW).decrypt(rotated) == b"legacy"
    with pytest.raises(InvalidToken):
        Fernet(OLD).decrypt(rotated)


def test_unknown_key_raises_invalid_token() -> None:
    stranger = Fernet(Fernet.generate_key()).encrypt(b"x")
    with pytest.raises(InvalidToken):
        Vault([NEW, OLD]).unseal(stranger)


def test_max_age_rejects_old_tokens() -> None:
    token = Fernet(NEW).encrypt_at_time(b"x", int(time.time()) - 100)
    vault = Vault([NEW, OLD])
    assert vault.unseal(token) == "x"
    with pytest.raises(InvalidToken):
        vault.unseal(token, max_age=10)


def test_a_key_is_required() -> None:
    with pytest.raises(ValueError, match="at least one key"):
        Vault([])
