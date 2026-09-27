"""Fixture for jti-based JWT revocation."""

import datetime as dt

import jwt
import pytest
from candidate import RevokedTokenError, TokenService

SECRET = "revocation-secret-0123456789abcdef"
TTL = dt.timedelta(minutes=10)


def test_revoked_token_stops_verifying() -> None:
    svc = TokenService(SECRET, TTL)
    token = svc.issue("alice")
    assert svc.verify(token)["sub"] == "alice"
    svc.revoke(token)
    with pytest.raises(RevokedTokenError):
        svc.verify(token)


def test_revocation_is_per_token() -> None:
    svc = TokenService(SECRET, TTL)
    first, second = svc.issue("alice"), svc.issue("alice")
    svc.revoke(first)
    assert svc.verify(second)["sub"] == "alice"


def test_revoke_is_idempotent() -> None:
    svc = TokenService(SECRET, TTL)
    token = svc.issue("alice")
    svc.revoke(token)
    svc.revoke(token)
    assert svc.revoked_count == 1


def test_purge_forgets_only_expired_revocations() -> None:
    svc = TokenService(SECRET, TTL)
    token = svc.issue("alice")
    exp = svc.verify(token)["exp"]
    svc.revoke(token)
    assert (svc.purge(exp - 1), svc.revoked_count) == (0, 1)
    assert (svc.purge(exp), svc.revoked_count) == (1, 0)


def test_jti_is_required() -> None:
    svc = TokenService(SECRET, TTL)
    exp = dt.datetime.now(dt.UTC) + TTL
    token = jwt.encode({"sub": "alice", "exp": exp}, SECRET, algorithm="HS256")
    with pytest.raises(jwt.MissingRequiredClaimError):
        svc.verify(token)


def test_secret_must_be_32_bytes() -> None:
    with pytest.raises(ValueError, match="32 bytes"):
        TokenService("x" * 31, TTL)
    assert TokenService("x" * 32, TTL).revoked_count == 0
