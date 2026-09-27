from datetime import UTC, datetime

import pytest
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.x509.oid import NameOID

from candidate import CertificateError, check_certificate

KEY = ec.generate_private_key(ec.SECP256R1())
OTHER = ec.generate_private_key(ec.SECP256R1())
START = datetime(2026, 1, 1, tzinfo=UTC)
END = datetime(2027, 1, 1, tzinfo=UTC)
MID = datetime(2026, 6, 1, tzinfo=UTC)


def name(cn):
    return x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, cn)])


def make(subject="api", issuer="api", signer=KEY, sans=("api.example.com",)):
    builder = (
        x509.CertificateBuilder()
        .subject_name(name(subject))
        .issuer_name(name(issuer))
        .public_key(KEY.public_key())
        .serial_number(1000)
        .not_valid_before(START)
        .not_valid_after(END)
    )
    if sans:
        ext = x509.SubjectAlternativeName([x509.DNSName(s) for s in sans])
        builder = builder.add_extension(ext, critical=False)
    cert = builder.sign(signer, hashes.SHA256())
    return cert.public_bytes(serialization.Encoding.PEM)


def test_valid_certificate_is_returned():
    pem = make(sans=("www.example.com", "api.example.com"))
    cert = check_certificate(pem, "API.Example.com", now=MID)
    assert isinstance(cert, x509.Certificate)
    assert cert.serial_number == 1000


def test_validity_window_is_inclusive():
    check_certificate(make(), "api.example.com", now=START)
    check_certificate(make(), "api.example.com", now=END)


def test_outside_window_is_rejected():
    with pytest.raises(CertificateError, match="not valid yet"):
        check_certificate(
            make(), "api.example.com", now=datetime(2025, 12, 31, tzinfo=UTC)
        )
    with pytest.raises(CertificateError, match="expired"):
        check_certificate(
            make(), "api.example.com", now=datetime(2027, 1, 2, tzinfo=UTC)
        )


def test_host_must_be_in_subject_alt_name():
    with pytest.raises(CertificateError, match="evil.example.com"):
        check_certificate(make(), "evil.example.com", now=MID)
    with pytest.raises(CertificateError, match="subjectAltName"):
        check_certificate(make(sans=()), "api", now=MID)


def test_certificate_must_be_self_signed():
    with pytest.raises(CertificateError, match="self-signed"):
        check_certificate(make(issuer="ca"), "api.example.com", now=MID)
    with pytest.raises(CertificateError, match="self-signed"):
        check_certificate(make(signer=OTHER), "api.example.com", now=MID)


def test_naive_now_is_rejected():
    with pytest.raises(ValueError, match="timezone-aware"):
        check_certificate(make(), "api.example.com", now=datetime(2026, 6, 1))
