import base64
import json
import pytest
from candidate import JWTAuthService

SECRET = "super-secret-crypto-key-32bytes!"


def test_valid_token_roundtrip():
    service = JWTAuthService(SECRET)
    token = service.create_token({"user": "alice", "role": "admin"})
    payload = service.decode_token(token)
    assert payload["user"] == "alice"
    assert payload["role"] == "admin"


def test_alg_none_token_rejected():
    service = JWTAuthService(SECRET)
    # Construct unsigned alg=none JWT (header.payload.)
    header_b64 = base64.urlsafe_b64encode(json.dumps({"alg": "none", "typ": "JWT"}).encode()).decode().rstrip("=")
    payload_b64 = base64.urlsafe_b64encode(json.dumps({"user": "attacker", "role": "superadmin"}).encode()).decode().rstrip("=")
    none_token = f"{header_b64}.{payload_b64}."

    with pytest.raises(ValueError, match="Invalid token"):
        service.decode_token(none_token)


def test_invalid_signature_rejected():
    service = JWTAuthService(SECRET)
    other_service = JWTAuthService("completely-different-signing-secret-key!")
    token_other = other_service.create_token({"user": "bob"})

    with pytest.raises(ValueError, match="Invalid token"):
        service.decode_token(token_other)


def test_forbidden_none_algorithm_init_rejected():
    with pytest.raises(ValueError, match="is forbidden"):
        JWTAuthService(SECRET, allowed_algorithms=["none"])
    with pytest.raises(ValueError, match="is forbidden"):
        JWTAuthService(SECRET, allowed_algorithms=["None"])


def test_short_secret_rejected():
    with pytest.raises(ValueError, match="Secret key must be at least"):
        JWTAuthService("short")
