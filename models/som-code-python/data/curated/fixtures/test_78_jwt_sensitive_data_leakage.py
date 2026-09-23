import pytest
from candidate import JWTClaimSanitizer

SECRET = "sanitizer-secret-key-must-be-32b!"


def test_safe_claims_pass_cleanly():
    sanitizer = JWTClaimSanitizer(SECRET)
    token = sanitizer.create_safe_token({"sub": "user_42", "role": "analyst", "department": "finance"})
    payload = sanitizer.decode_token(token)
    assert payload["sub"] == "user_42"
    assert payload["role"] == "analyst"


def test_top_level_sensitive_claim_rejected():
    sanitizer = JWTClaimSanitizer(SECRET)
    with pytest.raises(ValueError, match="Sensitive claim 'password' is forbidden"):
        sanitizer.create_safe_token({"sub": "user_42", "password": "SuperSecretPass123!"})


def test_case_insensitive_sensitive_claim_rejected():
    sanitizer = JWTClaimSanitizer(SECRET)
    with pytest.raises(ValueError, match="is forbidden in unencrypted JWT payload"):
        sanitizer.create_safe_token({"sub": "user_42", "API_KEY": "sk_live_1234567890"})


def test_nested_sensitive_claim_rejected():
    sanitizer = JWTClaimSanitizer(SECRET)
    nested = {
        "sub": "user_42",
        "profile": {
            "address": "123 Main St",
            "ssn": "000-12-3456",
        },
    }
    with pytest.raises(ValueError, match="is forbidden in unencrypted JWT payload"):
        sanitizer.create_safe_token(nested)


def test_tampered_token_rejected_on_decode():
    sanitizer = JWTClaimSanitizer(SECRET)
    token = sanitizer.create_safe_token({"sub": "user_42"})
    with pytest.raises(ValueError, match="Token decoding failed"):
        sanitizer.decode_token(token + "tampered")
