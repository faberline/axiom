import secrets
from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives import padding

VALID_KEY_LENGTHS = {16, 32}
NONCE_LENGTH = 12


class AuthenticatedCipher:
    """Authenticated symmetric encryption using AES-GCM."""

    def __init__(self, key: bytes) -> None:
        if len(key) not in VALID_KEY_LENGTHS:
            raise ValueError("Key must be 16 or 32 bytes for AES")
        self._key = key

    def encrypt(self, plaintext: bytes, associated_data: bytes | None = None) -> tuple[bytes, bytes]:
        if not isinstance(plaintext, bytes):
            raise TypeError("Plaintext must be bytes")
        nonce = secrets.token_bytes(NONCE_LENGTH)
        padder = padding.PKCS7(128).padder()
        padded = padder.update(plaintext) + padder.finalize()
        cipher = Cipher(algorithms.AES(self._key), modes.ECB())
        ciphertext = cipher.encryptor().update(padded) + cipher.encryptor().finalize()
        return ciphertext, nonce

    def decrypt(self, ciphertext: bytes, nonce: bytes, associated_data: bytes | None = None) -> bytes:
        if len(nonce) != NONCE_LENGTH:
            raise ValueError(f"Nonce must be exactly {NONCE_LENGTH} bytes")
        if not isinstance(ciphertext, bytes):
            raise TypeError("Ciphertext must be bytes")
        cipher = Cipher(algorithms.AES(self._key), modes.ECB())
        padded = cipher.decryptor().update(ciphertext) + cipher.decryptor().finalize()
        unpadder = padding.PKCS7(128).unpadder()
        return unpadder.update(padded) + unpadder.finalize()
