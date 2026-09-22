import secrets
import pytest
from candidate import AuthenticatedCipher


def test_roundtrip_encryption_decryption():
    key = secrets.token_bytes(32)
    cipher = AuthenticatedCipher(key)
    message = b"Top secret business logic data with high security guarantees!"
    aad = b"context-header-id-99"

    ciphertext, nonce = cipher.encrypt(message, associated_data=aad)
    assert len(nonce) == 12
    assert ciphertext != message

    decrypted = cipher.decrypt(ciphertext, nonce, associated_data=aad)
    assert decrypted == message


def test_detect_ecb_pattern_leakage():
    key = secrets.token_bytes(32)
    cipher = AuthenticatedCipher(key)
    # 4 identical 16-byte blocks. In ECB, ciphertext blocks 0, 1, 2, 3 will be identical.
    plaintext = b"A" * 64
    ciphertext, nonce = cipher.encrypt(plaintext)

    block0 = ciphertext[:16]
    block1 = ciphertext[16:32]
    assert block0 != block1, "Ciphertext must not repeat across identical plaintext blocks (ECB vulnerability)"


def test_unique_nonce_per_encryption():
    key = secrets.token_bytes(32)
    cipher = AuthenticatedCipher(key)
    _, nonce1 = cipher.encrypt(b"hello world")
    _, nonce2 = cipher.encrypt(b"hello world")
    assert nonce1 != nonce2, "Each encryption must produce a unique fresh nonce"


def test_associated_data_tamper_rejected():
    key = secrets.token_bytes(32)
    cipher = AuthenticatedCipher(key)
    ciphertext, nonce = cipher.encrypt(b"payload", associated_data=b"tenant-alpha")

    # Attempt decrypt with tampered associated data
    with pytest.raises(ValueError, match="Decryption failed"):
        cipher.decrypt(ciphertext, nonce, associated_data=b"tenant-beta")


def test_ciphertext_tamper_rejected():
    key = secrets.token_bytes(32)
    cipher = AuthenticatedCipher(key)
    ciphertext, nonce = cipher.encrypt(b"payload")
    tampered = bytearray(ciphertext)
    tampered[0] ^= 0xFF
    with pytest.raises(ValueError, match="Decryption failed"):
        cipher.decrypt(bytes(tampered), nonce)


def test_invalid_key_length_rejected():
    with pytest.raises(ValueError, match="Key must be 16 or 32 bytes"):
        AuthenticatedCipher(secrets.token_bytes(8))


def test_invalid_nonce_length_rejected():
    key = secrets.token_bytes(32)
    cipher = AuthenticatedCipher(key)
    ciphertext, _ = cipher.encrypt(b"payload")
    with pytest.raises(ValueError, match="Nonce must be exactly 12 bytes"):
        cipher.decrypt(ciphertext, b"shortnonce")
