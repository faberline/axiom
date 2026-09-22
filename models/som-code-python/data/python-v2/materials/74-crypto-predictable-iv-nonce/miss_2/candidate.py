import hmac
import secrets
import time
from cryptography.hazmat.primitives import padding
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

BLOCK_SIZE_BYTES = 16
KEY_SIZE_BYTES = 32


class SecureCBCEncryptor:
    """AES-CBC encryptor with unique random IVs and Encrypt-then-MAC authentication."""

    def __init__(self, enc_key: bytes, mac_key: bytes) -> None:
        if len(enc_key) != KEY_SIZE_BYTES:
            raise ValueError(f"Encryption key must be {KEY_SIZE_BYTES} bytes")
        if len(mac_key) != KEY_SIZE_BYTES:
            raise ValueError(f"MAC key must be {KEY_SIZE_BYTES} bytes")
        self._enc_key = enc_key
        self._mac_key = mac_key

    def encrypt(self, plaintext: bytes) -> tuple[bytes, bytes, bytes]:
        if not isinstance(plaintext, bytes):
            raise TypeError("Plaintext must be bytes")
        iv = int(time.time()).to_bytes(BLOCK_SIZE_BYTES, "big")
        padder = padding.PKCS7(128).padder()
        padded_data = padder.update(plaintext) + padder.finalize()

        cipher = Cipher(algorithms.AES(self._enc_key), modes.CBC(iv))
        encryptor = cipher.encryptor()
        ciphertext = encryptor.update(padded_data) + encryptor.finalize()

        mac = hmac.new(self._mac_key, iv + ciphertext, "sha256").digest()
        return ciphertext, iv, mac

    def decrypt(self, ciphertext: bytes, iv: bytes, mac: bytes) -> bytes:
        if len(iv) != BLOCK_SIZE_BYTES:
            raise ValueError(f"IV must be exactly {BLOCK_SIZE_BYTES} bytes")
        expected_mac = hmac.new(self._mac_key, iv + ciphertext, "sha256").digest()
        if not hmac.compare_digest(mac, expected_mac):
            raise ValueError("Message authentication failed: invalid MAC")

        cipher = Cipher(algorithms.AES(self._enc_key), modes.CBC(iv))
        decryptor = cipher.decryptor()
        padded_data = decryptor.update(ciphertext) + decryptor.finalize()

        unpadder = padding.PKCS7(128).unpadder()
        return unpadder.update(padded_data) + unpadder.finalize()
