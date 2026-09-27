import hmac

import pytest

import candidate
from candidate import TTL_SECONDS, ResetTokenError, ResetTokens


class Clock:
    def __init__(self):
        self.now = 1000.0

    def __call__(self):
        return self.now


@pytest.fixture
def clock():
    return Clock()


def test_token_redeems_once(clock):
    store = ResetTokens(clock)
    token = store.issue("ana")
    assert len(token) >= 43
    store.redeem("ana", token)
    with pytest.raises(ResetTokenError, match="invalid token"):
        store.redeem("ana", token)


def test_raw_token_is_never_stored(clock):
    store = ResetTokens(clock)
    token = store.issue("ana")
    assert token not in repr(vars(store))


def test_wrong_token_and_unknown_user_are_invalid(clock):
    store = ResetTokens(clock)
    token = store.issue("ana")
    with pytest.raises(ResetTokenError, match="invalid token"):
        store.redeem("ana", token + "x")
    with pytest.raises(ResetTokenError, match="invalid token"):
        store.redeem("bob", token)
    store.redeem("ana", token)


def test_reissue_invalidates_the_earlier_token(clock):
    store = ResetTokens(clock)
    old = store.issue("ana")
    new = store.issue("ana")
    with pytest.raises(ResetTokenError, match="invalid token"):
        store.redeem("ana", old)
    store.redeem("ana", new)


def test_token_expires_at_the_ttl_boundary(clock):
    assert TTL_SECONDS == 900
    store = ResetTokens(clock)
    token = store.issue("ana")
    clock.now += TTL_SECONDS
    with pytest.raises(ResetTokenError, match="token expired"):
        store.redeem("ana", token)
    with pytest.raises(ResetTokenError, match="invalid token"):
        store.redeem("ana", token)


def test_token_is_valid_just_before_expiry(clock):
    store = ResetTokens(clock)
    token = store.issue("ana")
    clock.now += TTL_SECONDS - 1
    store.redeem("ana", token)


def test_comparison_is_constant_time(clock, monkeypatch):
    real = hmac.compare_digest
    calls = []

    def spy(a, b):
        calls.append((a, b))
        return real(a, b)

    monkeypatch.setattr(candidate.hmac, "compare_digest", spy)
    store = ResetTokens(clock)
    token = store.issue("ana")
    store.redeem("ana", token)
    assert len(calls) == 1
