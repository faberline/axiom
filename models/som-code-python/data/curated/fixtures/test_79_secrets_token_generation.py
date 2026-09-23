import random
import pytest
from candidate import SecureTokenService, MIN_TOKEN_BYTES


def test_token_format_and_length():
    service = SecureTokenService()
    hex_tok = service.generate_hex_token(32)
    assert len(hex_tok) == 64
    int(hex_tok, 16)  # Validates hex string

    url_tok = service.generate_url_safe_token(32)
    assert len(url_tok) >= 32


def test_random_module_not_used_for_generation(monkeypatch):
    # Intercept random module methods to guarantee random is not called
    def forbidden_call(*args, **kwargs):
        raise RuntimeError("Insecure 'random' module was invoked instead of 'secrets'!")

    monkeypatch.setattr(random, "getrandbits", forbidden_call)
    monkeypatch.setattr(random, "choices", forbidden_call)
    monkeypatch.setattr(random, "randint", forbidden_call)

    service = SecureTokenService()
    tok = service.generate_hex_token(20)
    assert len(tok) == 40

    url_tok = service.generate_url_safe_token(20)
    assert len(url_tok) >= 20

    val = service.generate_bounded_int(100)
    assert 0 <= val < 100


def test_minimum_entropy_boundary_enforced():
    assert MIN_TOKEN_BYTES >= 16
    service = SecureTokenService()
    with pytest.raises(ValueError, match="nbytes must be at least"):
        service.generate_hex_token(4)

    with pytest.raises(ValueError, match="nbytes must be at least"):
        service.generate_url_safe_token(4)


def test_bounded_int_non_positive_rejected():
    service = SecureTokenService()
    with pytest.raises(ValueError, match="upper_bound must be positive"):
        service.generate_bounded_int(0)
    with pytest.raises(ValueError, match="upper_bound must be positive"):
        service.generate_bounded_int(-10)


def test_token_uniqueness_no_collisions():
    service = SecureTokenService()
    tokens = {service.generate_hex_token(16) for _ in range(50)}
    assert len(tokens) == 50, "No token collisions allowed across 50 generations"
