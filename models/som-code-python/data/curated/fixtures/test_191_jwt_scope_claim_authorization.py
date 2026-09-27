"""Fixture for scope-claim authorization."""

import jwt
import pytest
from candidate import InsufficientScopeError, require_scopes

SECRET = "scope-claim-secret-0123456789abcdef"


def token(scope: object = None) -> str:
    claims: dict[str, object] = {"sub": "alice"}
    if scope is not None:
        claims["scope"] = scope
    return jwt.encode(claims, SECRET, algorithm="HS256")


def test_all_needed_scopes_granted() -> None:
    tok = token("orders:read orders:write")
    claims = require_scopes(tok, SECRET, "orders:read", "orders:write")
    assert claims["sub"] == "alice"


def test_extra_whitespace_is_ignored() -> None:
    tok = token("  orders:read   users:read ")
    assert require_scopes(tok, SECRET, "users:read")["sub"] == "alice"


def test_missing_scopes_are_all_reported() -> None:
    with pytest.raises(InsufficientScopeError) as caught:
        require_scopes(
            token("orders:read"), SECRET, "orders:read", "orders:write", "users:read"
        )
    assert caught.value.missing == frozenset({"orders:write", "users:read"})
    assert str(caught.value) == "missing scopes: orders:write users:read"


def test_scope_prefix_is_not_a_grant() -> None:
    with pytest.raises(InsufficientScopeError):
        require_scopes(token("orders:readonly"), SECRET, "orders:read")


def test_no_scope_claim_grants_nothing() -> None:
    with pytest.raises(InsufficientScopeError):
        require_scopes(token(), SECRET, "orders:read")


def test_list_scope_is_a_type_error() -> None:
    with pytest.raises(TypeError, match="space-separated"):
        require_scopes(token(["orders:read"]), SECRET, "orders:read")


def test_some_scope_must_be_required() -> None:
    with pytest.raises(ValueError, match="at least one"):
        require_scopes(token("orders:read"), SECRET)


def test_bad_signature_is_not_swallowed() -> None:
    forged = jwt.encode(
        {"scope": "orders:read"}, "another-secret-0123456789abcdefgh", algorithm="HS256"
    )
    with pytest.raises(jwt.InvalidSignatureError):
        require_scopes(forged, SECRET, "orders:read")
