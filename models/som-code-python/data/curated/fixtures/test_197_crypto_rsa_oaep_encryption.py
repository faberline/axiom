import pytest
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import padding, rsa

from candidate import DecryptionError, decrypt, encrypt, max_plaintext

KEY = rsa.generate_private_key(public_exponent=65537, key_size=2048)
OTHER = rsa.generate_private_key(public_exponent=65537, key_size=2048)
WEAK = rsa.generate_private_key(public_exponent=65537, key_size=1024)
OAEP = padding.OAEP(
    mgf=padding.MGF1(algorithm=hashes.SHA256()),
    algorithm=hashes.SHA256(),
    label=None,
)


def test_ciphertext_is_standard_oaep_sha256():
    token = encrypt(KEY.public_key(), b"api-secret")
    assert len(token) == 256
    assert KEY.decrypt(token, OAEP) == b"api-secret"
    assert token != encrypt(KEY.public_key(), b"api-secret")


def test_round_trip_through_decrypt():
    assert decrypt(KEY, KEY.public_key().encrypt(b"s3cret", OAEP)) == b"s3cret"


def test_limit_is_190_bytes_for_2048_bit_keys():
    assert max_plaintext(KEY.public_key()) == 190
    assert decrypt(KEY, encrypt(KEY.public_key(), b"x" * 190)) == b"x" * 190
    with pytest.raises(ValueError, match="191 bytes, limit is 190"):
        encrypt(KEY.public_key(), b"x" * 191)


def test_weak_keys_are_refused():
    with pytest.raises(ValueError, match="2048 bits"):
        encrypt(WEAK.public_key(), b"hi")


def test_wrong_key_or_tampering_raises_decryption_error():
    token = encrypt(KEY.public_key(), b"hello")
    with pytest.raises(DecryptionError, match="failed"):
        decrypt(OTHER, token)
    tampered = bytes([token[0] ^ 1]) + token[1:]
    with pytest.raises(DecryptionError, match="failed"):
        decrypt(KEY, tampered)


def test_wrong_length_ciphertext_is_rejected():
    with pytest.raises(DecryptionError, match="wrong length"):
        decrypt(KEY, b"\x00" * 128)
