import pytest
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import padding, rsa

from candidate import sign_stream, verify_stream

KEY = rsa.generate_private_key(public_exponent=65537, key_size=2048)
OTHER = rsa.generate_private_key(public_exponent=65537, key_size=2048)
WEAK = rsa.generate_private_key(public_exponent=65537, key_size=1024)
DATA = b"release-artifact " * 500
PSS = padding.PSS(mgf=padding.MGF1(hashes.SHA256()), salt_length=32)


def chunks(data, size):
    return (data[i : i + size] for i in range(0, len(data), size))


def test_signature_is_standard_pss_sha256_over_the_whole_stream():
    sig = sign_stream(KEY, chunks(DATA, 1000))
    assert len(sig) == 256
    KEY.public_key().verify(sig, DATA, PSS, hashes.SHA256())


def test_chunking_does_not_matter_for_verification():
    sig = sign_stream(KEY, [DATA])
    assert verify_stream(KEY.public_key(), chunks(DATA, 7), sig)


def test_accepts_a_signature_made_by_the_reference():
    sig = KEY.sign(DATA, PSS, hashes.SHA256())
    assert verify_stream(KEY.public_key(), [DATA], sig)


def test_tampering_and_wrong_keys_return_false():
    sig = sign_stream(KEY, [DATA])
    assert not verify_stream(KEY.public_key(), [DATA + b"!"], sig)
    assert not verify_stream(OTHER.public_key(), [DATA], sig)
    assert not verify_stream(KEY.public_key(), [DATA], sig[:-1])
    assert not verify_stream(KEY.public_key(), [DATA], bytes(256))


def test_weak_signing_keys_are_refused():
    with pytest.raises(ValueError, match="2048 bits"):
        sign_stream(WEAK, [DATA])
