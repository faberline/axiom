import time
import jwt
import pytest
from candidate import ExpirableTokenManager

SECRET = "expiration-validation-secret-key-32b!"


def test_valid_unexpired_token():
    manager = ExpirableTokenManager(SECRET, leeway_seconds=5)
    token = manager.issue_token(subject="user-active", ttl_seconds=120)
    payload = manager.decode_token(token)
    assert payload["sub"] == "user-active"
    assert payload["exp"] > int(time.time())


def test_expired_token_raises_value_error():
    manager = ExpirableTokenManager(SECRET, leeway_seconds=5)
    now = int(time.time())
    # Create token expired 60 seconds ago
    expired_token = jwt.encode({"sub": "user-old", "iat": now - 3600, "exp": now - 60}, SECRET, algorithm="HS256")
    with pytest.raises(ValueError, match="Token has expired"):
        manager.decode_token(expired_token)


def test_token_missing_exp_claim_rejected():
    manager = ExpirableTokenManager(SECRET, leeway_seconds=5)
    token_no_exp = jwt.encode({"sub": "user-forever", "iat": int(time.time())}, SECRET, algorithm="HS256")
    with pytest.raises(ValueError, match="Token validation failed"):
        manager.decode_token(token_no_exp)


def test_excessive_leeway_rejected_in_init():
    with pytest.raises(ValueError, match="Leeway must be between 0 and 30 seconds"):
        ExpirableTokenManager(SECRET, leeway_seconds=3600)


def test_negative_ttl_rejected():
    manager = ExpirableTokenManager(SECRET)
    with pytest.raises(ValueError, match="TTL must be greater than 0"):
        manager.issue_token(subject="user-invalid", ttl_seconds=-10)
