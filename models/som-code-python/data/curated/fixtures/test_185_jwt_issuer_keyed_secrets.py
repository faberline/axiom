"""Fixture for 185: per-issuer secrets picked by claim, then verified."""

import jwt
import pytest

from candidate import UntrustedIssuerError, issue, verify

SECRETS = {
    "https://a.example": "a" * 32 + "-issuer-a-signing-secret",
    "https://b.example": "b" * 32 + "-issuer-b-signing-secret",
}
AUD = "orders-api"


def test_each_issuer_verifies_with_its_own_secret() -> None:
    for issuer, secret in SECRETS.items():
        token = issue(secret, issuer, "user-1", AUD)
        claims = verify(token, SECRETS, AUD)
        assert claims == {"iss": issuer, "sub": "user-1", "aud": AUD}


def test_token_signed_with_another_issuers_secret_is_rejected() -> None:
    forged = issue(SECRETS["https://a.example"], "https://b.example", "admin", AUD)
    with pytest.raises(jwt.InvalidSignatureError):
        verify(forged, SECRETS, AUD)


def test_unknown_issuer_is_untrusted() -> None:
    token = issue("c" * 40, "https://evil.example", "user-1", AUD)
    with pytest.raises(UntrustedIssuerError, match="evil.example"):
        verify(token, SECRETS, AUD)
    assert issubclass(UntrustedIssuerError, jwt.InvalidTokenError)


def test_missing_issuer_is_untrusted() -> None:
    token = jwt.encode({"sub": "user-1", "aud": AUD}, "d" * 40, algorithm="HS256")
    with pytest.raises(UntrustedIssuerError, match="None"):
        verify(token, SECRETS, AUD)


def test_only_hs256_is_accepted() -> None:
    issuer = "https://a.example"
    claims = {"iss": issuer, "sub": "user-1", "aud": AUD}
    token = jwt.encode(claims, SECRETS[issuer] * 2, algorithm="HS512")
    with pytest.raises(jwt.InvalidAlgorithmError):
        verify(token, {issuer: SECRETS[issuer] * 2}, AUD)


def test_subject_is_required() -> None:
    issuer = "https://b.example"
    token = jwt.encode({"iss": issuer, "aud": AUD}, SECRETS[issuer], algorithm="HS256")
    with pytest.raises(jwt.MissingRequiredClaimError, match="sub"):
        verify(token, SECRETS, AUD)


def test_wrong_audience_and_garbage_are_rejected() -> None:
    issuer = "https://a.example"
    token = issue(SECRETS[issuer], issuer, "user-1", "billing-api")
    with pytest.raises(jwt.InvalidAudienceError):
        verify(token, SECRETS, AUD)
    with pytest.raises(jwt.DecodeError):
        verify("not-a-token", SECRETS, AUD)
