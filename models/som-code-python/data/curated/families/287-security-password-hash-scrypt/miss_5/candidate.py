"""Hash and verify passwords with hashlib.scrypt in a self-describing format."""

from __future__ import annotations

import base64
import hashlib
import hmac
import secrets

N, R, P = 2**14, 8, 1
SALT_BYTES = 16
KEY_BYTES = 32


class HashFormatError(ValueError):
    """Raised when a stored hash string cannot be parsed."""


def _b64(data: bytes) -> str:
    return base64.b64encode(data).decode("ascii")


def _unb64(text: str) -> bytes:
    return base64.b64decode(text, validate=True)


def _derive(password: str, salt: bytes, params: tuple[int, int, int]) -> bytes:
    n, r, p = params
    return hashlib.scrypt(
        password.encode("utf-8"), salt=salt, n=n, r=r, p=p, dklen=KEY_BYTES
    )


def hash_password(password: str, params: tuple[int, int, int] = (N, R, P)) -> str:
    """Return 'scrypt$n$r$p$salt$key' with a fresh random salt."""
    if not password:
        raise ValueError("password must not be empty")
    salt = secrets.token_bytes(SALT_BYTES)
    key = _derive(password, salt, params)
    return "$".join(["scrypt", *map(str, params), _b64(salt), _b64(key)])


def parse_hash(stored: str) -> tuple[tuple[int, int, int], bytes, bytes]:
    """Split a stored hash into its cost parameters, salt and key."""
    parts = stored.split("$")
    if len(parts) != 6 or parts[0] != "scrypt":
        raise HashFormatError("not an scrypt hash")
    try:
        n, r, p = (int(part) for part in parts[1:4])
        salt, key = _unb64(parts[4]), _unb64(parts[5])
    except ValueError as exc:
        raise HashFormatError("malformed scrypt hash") from exc
    if n < 2 or n & (n - 1):
        raise HashFormatError("n must be a power of two")
    return (n, r, p), salt, key


def verify_password(password: str, stored: str) -> bool:
    """Return whether the password matches, comparing in constant time."""
    params, salt, key = parse_hash(stored)
    return hmac.compare_digest(_derive(password, salt, params), key)


def needs_rehash(stored: str) -> bool:
    """Return whether the stored hash uses cost parameters other than the current."""
    params, _, _ = parse_hash(stored)
    return params[0] != N
