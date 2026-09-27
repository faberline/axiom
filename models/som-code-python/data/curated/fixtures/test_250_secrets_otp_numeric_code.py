import hmac

import pytest

import candidate
from candidate import OtpChallenge, OtpLockedError, generate_code


def test_codes_are_six_digits_by_default():
    for _ in range(200):
        code = generate_code()
        assert len(code) == 6
        assert code.isdigit()


def test_codes_are_zero_padded_from_randbelow(monkeypatch):
    seen = []

    def fake(n):
        seen.append(n)
        return 42

    monkeypatch.setattr(candidate.secrets, "randbelow", fake)
    assert generate_code() == "000042"
    assert generate_code(8) == "00000042"
    assert seen == [10**6, 10**8]


def test_digit_count_is_validated():
    assert generate_code(4).isdigit()
    assert generate_code(10).isdigit()
    for bad in (3, 11, 0):
        with pytest.raises(ValueError, match="between 4 and 10"):
            generate_code(bad)


def test_correct_code_is_accepted_once():
    challenge = OtpChallenge("123456")
    assert challenge.check("123456") is True
    assert challenge.remaining() == 0
    with pytest.raises(OtpLockedError):
        challenge.check("123456")


def test_three_wrong_attempts_lock_the_challenge():
    challenge = OtpChallenge("123456")
    assert challenge.check("000000") is False
    assert challenge.check("111111") is False
    assert challenge.remaining() == 1
    assert challenge.check("222222") is False
    assert challenge.remaining() == 0
    with pytest.raises(OtpLockedError, match="challenge closed"):
        challenge.check("123456")


def test_comparison_is_constant_time(monkeypatch):
    real = hmac.compare_digest
    calls = []

    def spy(a, b):
        calls.append((a, b))
        return real(a, b)

    monkeypatch.setattr(candidate.hmac, "compare_digest", spy)
    assert OtpChallenge("654321").check("654321") is True
    assert calls == [(b"654321", b"654321")]
