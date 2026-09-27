import pytest
from candidate import PBKDF2KeyDeriver, MIN_RECOMMENDED_ITERATIONS


def test_default_min_iterations_standard():
    assert MIN_RECOMMENDED_ITERATIONS >= 600_000
    deriver = PBKDF2KeyDeriver()
    salt = deriver.generate_salt(16)
    # Run with 600_000 iterations to verify standard compliance
    key = deriver.derive_key("valid_passphrase", salt, iterations=600_000, dklen=32)
    assert len(key) == 32


def test_iterations_below_threshold_rejected():
    deriver = PBKDF2KeyDeriver()
    salt = deriver.generate_salt(16)
    with pytest.raises(ValueError, match="below required"):
        deriver.derive_key("valid_passphrase", salt, iterations=50_000)


def test_short_salt_rejected():
    deriver = PBKDF2KeyDeriver()
    with pytest.raises(ValueError, match="Salt must be at least"):
        deriver.derive_key("valid_passphrase", salt=b"short", iterations=600_000)


def test_custom_dklen_respected():
    deriver = PBKDF2KeyDeriver()
    salt = deriver.generate_salt(16)
    key16 = deriver.derive_key("passphrase", salt, iterations=600_000, dklen=16)
    key64 = deriver.derive_key("passphrase", salt, iterations=600_000, dklen=64)
    assert len(key16) == 16
    assert len(key64) == 64


def test_empty_passphrase_rejected():
    deriver = PBKDF2KeyDeriver()
    salt = deriver.generate_salt(16)
    with pytest.raises(ValueError, match="Passphrase cannot be empty"):
        deriver.derive_key("", salt, iterations=600_000)


def test_generate_salt_validates_length():
    deriver = PBKDF2KeyDeriver()
    with pytest.raises(ValueError, match="Salt length must be at least"):
        deriver.generate_salt(8)
    assert len(deriver.generate_salt(24)) == 24
