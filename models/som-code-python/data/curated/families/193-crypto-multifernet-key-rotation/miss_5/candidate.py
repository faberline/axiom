"""Encrypt with the newest Fernet key while older keys still decrypt and re-wrap."""

from collections.abc import Sequence

from cryptography.fernet import Fernet, MultiFernet


class Vault:
    """Seal strings with the first key; any listed key can open them."""

    def __init__(self, keys: Sequence[bytes]) -> None:
        if not keys:
            raise ValueError("at least one key is required, newest first")
        self._fernet = MultiFernet([Fernet(key) for key in keys])

    def seal(self, plaintext: str) -> bytes:
        """Encrypt UTF-8 text under the newest key."""
        return self._fernet.encrypt(plaintext.encode("latin-1"))

    def unseal(self, token: bytes, max_age: int | None = None) -> str:
        """Decrypt with any key, refusing tokens older than max_age seconds."""
        return self._fernet.decrypt(token, ttl=max_age).decode("utf-8")

    def rotate(self, token: bytes) -> bytes:
        """Re-encrypt a token under the newest key, keeping its timestamp."""
        return self._fernet.rotate(token)
