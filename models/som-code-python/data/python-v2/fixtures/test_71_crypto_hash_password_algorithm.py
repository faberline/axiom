import hashlib
import hmac
import pytest
from candidate import PasswordHasher


def test_hash_password_success_and_verify():
    hasher = PasswordHasher(iterations=100_000)
    pwd = "CorrectHorseBatteryStaple123!"
    hash_hex, salt = hasher.hash_password(pwd)

    assert len(salt) == 16
    assert len(hash_hex) == 64
    assert hasher.verify_password(pwd, hash_hex, salt) is True
    assert hasher.verify_password("WrongPassword!", hash_hex, salt) is False


def test_hash_password_random_salt():
    hasher = PasswordHasher(iterations=100_000)
    pwd = "MySecretPassword456!"
    hash1, salt1 = hasher.hash_password(pwd)
    hash2, salt2 = hasher.hash_password(pwd)

    assert salt1 != salt2, "Salt must be randomly generated for each invocation"
    assert hash1 != hash2, "Hashes must differ due to unique salts"


def test_rejects_md5_and_sha1_digests():
    hasher = PasswordHasher(iterations=100_000)
    pwd = "SampleTestPassword789!"
    hash_hex, salt = hasher.hash_password(pwd)

    assert len(hash_hex) == 64
    md5_hash = hashlib.md5(salt + pwd.encode("utf-8")).hexdigest()
    sha1_hash = hashlib.sha1(salt + pwd.encode("utf-8")).hexdigest()
    assert hash_hex != md5_hash, "Must not use MD5 digest"
    assert hash_hex != sha1_hash, "Must not use SHA1 digest"


def test_empty_password_rejected():
    hasher = PasswordHasher(iterations=100_000)
    with pytest.raises(ValueError, match="Password cannot be empty"):
        hasher.hash_password("")


def test_constant_time_comparison_enforced(monkeypatch):
    hasher = PasswordHasher(iterations=100_000)
    pwd = "TimingTestPassword!"
    hash_hex, salt = hasher.hash_password(pwd)

    called = False
    original_compare_digest = hmac.compare_digest

    def spy_compare_digest(a, b):
        nonlocal called
        called = True
        return original_compare_digest(a, b)

    monkeypatch.setattr(hmac, "compare_digest", spy_compare_digest)
    res = hasher.verify_password(pwd, hash_hex, salt)
    assert res is True
    assert called is True, "Must verify using constant-time hmac.compare_digest"


def test_short_salt_rejected():
    hasher = PasswordHasher(iterations=100_000)
    with pytest.raises(ValueError, match="Salt must be at least"):
        hasher.hash_password("password", salt=b"tooshort")


def test_low_iterations_rejected():
    with pytest.raises(ValueError, match="Iterations must be at least"):
        PasswordHasher(iterations=50_000)
