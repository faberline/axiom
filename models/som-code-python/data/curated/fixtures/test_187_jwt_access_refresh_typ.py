"""Fixture for keeping access and refresh tokens apart."""

import datetime as dt

import jwt
import pytest
from candidate import WrongTokenTypeError, issue_pair, read_token, refresh_access

SECRET = "access-refresh-secret-0123456789abcdef"


def test_pair_carries_types_and_lifetimes() -> None:
    access, refresh = issue_pair(SECRET, "alice")
    a = read_token(SECRET, access, "access")
    r = read_token(SECRET, refresh, "refresh")
    assert (a["sub"], a["typ"], a["exp"] - a["iat"]) == ("alice", "access", 900)
    assert (r["sub"], r["typ"], r["exp"] - r["iat"]) == ("alice", "refresh", 604800)


def test_tokens_are_not_interchangeable() -> None:
    access, refresh = issue_pair(SECRET, "alice")
    with pytest.raises(WrongTokenTypeError):
        read_token(SECRET, refresh, "access")
    with pytest.raises(WrongTokenTypeError):
        read_token(SECRET, access, "refresh")


def test_refresh_mints_a_new_access_token() -> None:
    _, refresh = issue_pair(SECRET, "alice")
    claims = read_token(SECRET, refresh_access(SECRET, refresh), "access")
    assert (claims["sub"], claims["exp"] - claims["iat"]) == ("alice", 900)


def test_access_token_cannot_refresh() -> None:
    access, _ = issue_pair(SECRET, "alice")
    with pytest.raises(WrongTokenTypeError):
        refresh_access(SECRET, access)


def test_typ_is_required() -> None:
    exp = dt.datetime.now(dt.UTC) + dt.timedelta(minutes=1)
    token = jwt.encode({"sub": "alice", "exp": exp}, SECRET, algorithm="HS256")
    with pytest.raises(jwt.MissingRequiredClaimError):
        read_token(SECRET, token, "access")


def test_other_secret_fails_the_signature() -> None:
    access, _ = issue_pair("another-secret-0123456789abcdefghij", "alice")
    with pytest.raises(jwt.InvalidSignatureError):
        read_token(SECRET, access, "access")


def test_empty_subject_is_rejected() -> None:
    with pytest.raises(ValueError, match="subject"):
        issue_pair(SECRET, "")
