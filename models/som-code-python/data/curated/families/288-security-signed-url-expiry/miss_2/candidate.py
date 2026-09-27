"""Sign and verify expiring download URLs with an HMAC over the path and query."""

from __future__ import annotations

import hashlib
import hmac
from urllib.parse import parse_qsl, urlencode, urlsplit, urlunsplit


class SignatureError(Exception):
    """Raised when a URL is unsigned, tampered with or expired."""


def _digest(secret: bytes, path: str, params: list[tuple[str, str]]) -> str:
    message = path + "?" + urlencode(sorted(params))
    return hmac.new(secret, message.encode("utf-8"), hashlib.sha256).hexdigest()


def sign_url(url: str, secret: bytes, expires_at: int) -> str:
    """Return url with 'expires' and 'sig' query parameters appended."""
    parts = urlsplit(url)
    params = parse_qsl(parts.query, keep_blank_values=True)
    if any(key in ("expires", "sig") for key, _ in params):
        raise ValueError("url already carries signing parameters")
    params.append(("expires", str(expires_at)))
    signature = _digest(secret, parts.path, params)
    query = urlencode([*params, ("sig", signature)])
    return urlunsplit(parts._replace(query=query))


def verify_url(url: str, secret: bytes, now: int) -> dict[str, str]:
    """Return the original query parameters of a valid, unexpired signed URL."""
    parts = urlsplit(url)
    params = parse_qsl(parts.query, keep_blank_values=True)
    signatures = [value for key, value in params if key == "sig"]
    if len(signatures) != 1:
        raise SignatureError("missing or repeated signature")
    unsigned = [(key, value) for key, value in params if key != "sig"]
    expected = _digest(secret, parts.path, unsigned)
    if not hmac.compare_digest(expected, signatures[0]):
        raise SignatureError("bad signature")
    expires = dict(unsigned).get("expires", "")
    if not expires.isdigit() or now > int(expires):
        raise SignatureError("link expired")
    return {key: value for key, value in unsigned if key != "expires"}
