import pytest
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

from candidate import WrongPassphraseError, export_key, import_key

KEY = Ed25519PrivateKey.from_private_bytes(bytes(range(32)))
PHRASE = "grüne-äpfel-im-korb"


def raw(key):
    return key.private_bytes(
        serialization.Encoding.Raw,
        serialization.PrivateFormat.Raw,
        serialization.NoEncryption(),
    )


def test_export_is_encrypted_pkcs8_readable_by_the_library():
    pem = export_key(KEY, PHRASE)
    assert pem.startswith(b"-----BEGIN ENCRYPTED PRIVATE KEY-----")
    loaded = serialization.load_pem_private_key(pem, password=PHRASE.encode())
    assert raw(loaded) == raw(KEY)


def test_round_trip_restores_the_same_key():
    assert raw(import_key(export_key(KEY, PHRASE), PHRASE)) == raw(KEY)


def test_passphrase_length_limit_is_12_characters():
    export_key(KEY, "x" * 12)
    with pytest.raises(ValueError, match="at least 12"):
        export_key(KEY, "x" * 11)


def test_wrong_passphrase_raises_wrong_passphrase_error():
    pem = export_key(KEY, PHRASE)
    with pytest.raises(WrongPassphraseError):
        import_key(pem, "not-the-right-one")


def test_unencrypted_pem_is_refused():
    plain = KEY.private_bytes(
        serialization.Encoding.PEM,
        serialization.PrivateFormat.PKCS8,
        serialization.NoEncryption(),
    )
    with pytest.raises(ValueError, match="not encrypted") as info:
        import_key(plain, PHRASE)
    assert not isinstance(info.value, WrongPassphraseError)


def test_other_key_types_raise_type_error():
    other = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    pem = other.private_bytes(
        serialization.Encoding.PEM,
        serialization.PrivateFormat.PKCS8,
        serialization.BestAvailableEncryption(PHRASE.encode()),
    )
    with pytest.raises(TypeError, match="Ed25519"):
        import_key(pem, PHRASE)
