import jwt
import pytest
from candidate import AudienceTokenVerifier

SECRET = "jwt-audience-verifier-secret-key-32b!"
TARGET_AUD = "microservice-payment"
ISSUER = "auth-service-core"


def test_valid_token_audience_and_issuer_success():
    verifier = AudienceTokenVerifier(SECRET, expected_audience=TARGET_AUD, expected_issuer=ISSUER)
    token = verifier.issue_token(subject="user-123")
    claims = verifier.verify_token(token)
    assert claims["sub"] == "user-123"
    assert claims["aud"] == TARGET_AUD
    assert claims["iss"] == ISSUER


def test_foreign_audience_rejected():
    verifier = AudienceTokenVerifier(SECRET, expected_audience=TARGET_AUD, expected_issuer=ISSUER)
    foreign_token = verifier.issue_token(subject="user-123", audience="microservice-analytics")
    with pytest.raises(ValueError, match="verification failed"):
        verifier.verify_token(foreign_token)


def test_missing_audience_claim_rejected():
    verifier = AudienceTokenVerifier(SECRET, expected_audience=TARGET_AUD, expected_issuer=ISSUER)
    # Issue token without aud claim directly
    token_no_aud = jwt.encode({"sub": "user-123", "iss": ISSUER}, SECRET, algorithm="HS256")
    with pytest.raises(ValueError, match="verification failed"):
        verifier.verify_token(token_no_aud)


def test_substring_audience_spoofing_rejected():
    verifier = AudienceTokenVerifier(SECRET, expected_audience=TARGET_AUD, expected_issuer=ISSUER)
    # Attacker crafts token for microservice-payment-attacker
    spoofed_token = verifier.issue_token(subject="user-123", audience=f"{TARGET_AUD}-spoofed-extra")
    with pytest.raises(ValueError, match="verification failed"):
        verifier.verify_token(spoofed_token)


def test_foreign_issuer_rejected():
    verifier = AudienceTokenVerifier(SECRET, expected_audience=TARGET_AUD, expected_issuer=ISSUER)
    # Token issued with different issuer
    foreign_iss_token = jwt.encode({"sub": "user-123", "aud": TARGET_AUD, "iss": "rogue-issuer"}, SECRET, algorithm="HS256")
    with pytest.raises(ValueError, match="verification failed"):
        verifier.verify_token(foreign_iss_token)


def test_empty_audience_or_issuer_rejected_in_init():
    with pytest.raises(ValueError, match="Expected audience cannot be empty"):
        AudienceTokenVerifier(SECRET, expected_audience="", expected_issuer=ISSUER)
    with pytest.raises(ValueError, match="Expected issuer cannot be empty"):
        AudienceTokenVerifier(SECRET, expected_audience=TARGET_AUD, expected_issuer="")
