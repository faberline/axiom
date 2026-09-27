"""Fixture for Bearer header parsing and JWT verification."""

import jwt
import pytest
from candidate import AuthError, authenticate

SECRET = "bearer-header-secret-0123456789abcdef"


def token(secret: str = SECRET, aud: str = "orders") -> str:
    return jwt.encode({"sub": "alice", "aud": aud}, secret, algorithm="HS256")


def failure(header: str | None) -> tuple[int, str]:
    with pytest.raises(AuthError) as caught:
        authenticate(header, SECRET, "orders")
    return caught.value.status, caught.value.detail


def test_valid_header_returns_claims() -> None:
    claims = authenticate(f"Bearer {token()}", SECRET, "orders")
    assert claims == {"sub": "alice", "aud": "orders"}


def test_scheme_is_case_insensitive_and_spaces_are_trimmed() -> None:
    assert authenticate(f"bearer {token()}", SECRET, "orders")["sub"] == "alice"
    assert authenticate(f"  Bearer   {token()} ", SECRET, "orders")["sub"] == "alice"


def test_missing_header_is_401() -> None:
    assert failure(None) == (401, "missing authorization header")


def test_other_scheme_is_401() -> None:
    assert failure("Basic YWxpY2U6cHc=") == (401, "authorization scheme must be Bearer")


def test_malformed_token_is_401() -> None:
    assert failure("Bearer a b") == (401, "malformed bearer token")
    assert failure("Bearer ") == (401, "malformed bearer token")


def test_bad_signature_is_401() -> None:
    other = token(secret="another-secret-0123456789abcdefghij")
    assert failure(f"Bearer {other}") == (401, "invalid token")


def test_wrong_audience_is_401() -> None:
    assert failure(f"Bearer {token(aud='billing')}") == (401, "invalid token")
