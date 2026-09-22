import secrets
import pytest
from candidate import SecureCBCEncryptor


def test_roundtrip_cbc_encryption():
    enc_key = secrets.token_bytes(32)
    mac_key = secrets.token_bytes(32)
    encryptor = SecureCBCEncryptor(enc_key, mac_key)

    msg = b"Confidential financial transaction record payload"
    ciphertext, iv, mac = encryptor.encrypt(msg)

    assert len(iv) == 16, "AES IV must be 16 bytes"
    assert len(mac) == 32, "HMAC-SHA256 must be 32 bytes"

    decrypted = encryptor.decrypt(ciphertext, iv, mac)
    assert decrypted == msg


def test_iv_uniqueness_and_non_predictability():
    enc_key = secrets.token_bytes(32)
    mac_key = secrets.token_bytes(32)
    encryptor = SecureCBCEncryptor(enc_key, mac_key)

    ivs = [encryptor.encrypt(b"repeat message")[1] for _ in range(5)]
    assert len(set(ivs)) == 5, "IVs must be unique across invocations"

    # Verify not a simple sequential counter (e.g. 0x01, 0x02)
    int_ivs = [int.from_bytes(iv, "big") for iv in ivs]
    differences = [int_ivs[i + 1] - int_ivs[i] for i in range(len(int_ivs) - 1)]
    assert not all(d == 1 for d in differences), "IV must not be sequential counter"

    # Verify high entropy (not mostly zeroes)
    for iv in ivs:
        assert iv.count(b"\x00") < 8, "IV must have high entropy and not be zero-padded counter or timestamp"


def test_tampered_mac_rejected():
    enc_key = secrets.token_bytes(32)
    mac_key = secrets.token_bytes(32)
    encryptor = SecureCBCEncryptor(enc_key, mac_key)

    ciphertext, iv, mac = encryptor.encrypt(b"hello")
    bad_mac = bytearray(mac)
    bad_mac[0] ^= 0x01
    with pytest.raises(ValueError, match="Message authentication failed"):
        encryptor.decrypt(ciphertext, iv, bytes(bad_mac))


def test_tampered_ciphertext_fails_mac():
    enc_key = secrets.token_bytes(32)
    mac_key = secrets.token_bytes(32)
    encryptor = SecureCBCEncryptor(enc_key, mac_key)

    ciphertext, iv, mac = encryptor.encrypt(b"hello")
    bad_ct = bytearray(ciphertext)
    bad_ct[0] ^= 0x01
    with pytest.raises(ValueError, match="Message authentication failed"):
        encryptor.decrypt(bytes(bad_ct), iv, mac)


def test_invalid_key_lengths():
    with pytest.raises(ValueError, match="Encryption key must be 32 bytes"):
        SecureCBCEncryptor(b"short_enc_key", secrets.token_bytes(32))
    with pytest.raises(ValueError, match="MAC key must be 32 bytes"):
        SecureCBCEncryptor(secrets.token_bytes(32), b"short_mac_key")
