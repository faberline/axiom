"""Generate passwords that satisfy a character-class policy with secrets."""

from __future__ import annotations

import secrets
import string

MIN_LENGTH = 12
SYMBOLS = "!@#$%^&*-_"
CLASSES = (string.ascii_lowercase, string.ascii_uppercase, string.digits, SYMBOLS)
ALPHABET = "".join(CLASSES)


def generate_password(length: int = 16) -> str:
    """Return a random password containing every character class."""
    if length < MIN_LENGTH:
        raise ValueError(f"length must be at least {MIN_LENGTH}")
    chars = [secrets.choice(cls) for cls in CLASSES]
    chars += [secrets.choice(ALPHABET) for _ in range(length - len(chars))]
    return "".join(chars)
