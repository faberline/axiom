"""Check a self-signed PEM certificate before pinning it for a host."""

from __future__ import annotations

from datetime import datetime

from cryptography import x509
from cryptography.exceptions import InvalidSignature


class CertificateError(ValueError):
    """The certificate cannot be pinned for this host."""


def check_certificate(pem: bytes, hostname: str, *, now: datetime) -> x509.Certificate:
    """Load ``pem`` and require it to be current, self-signed and name ``hostname``."""
    if now.tzinfo is None:
        raise ValueError("now must be timezone-aware")
    cert = x509.load_pem_x509_certificate(pem)
    if now < cert.not_valid_before_utc:
        raise CertificateError("certificate is not valid yet")
    if now > cert.not_valid_after_utc:
        raise CertificateError("certificate has expired")
    try:
        san = cert.extensions.get_extension_for_class(x509.SubjectAlternativeName)
    except x509.ExtensionNotFound as exc:
        raise CertificateError("certificate has no subjectAltName") from exc
    names = set(san.value.get_values_for_type(x509.DNSName))
    if hostname not in names:
        raise CertificateError(f"{hostname} is not in subjectAltName")
    try:
        cert.verify_directly_issued_by(cert)
    except (ValueError, InvalidSignature) as exc:
        raise CertificateError("certificate is not self-signed") from exc
    return cert
