from urllib.parse import parse_qsl, urlencode, urlsplit, urlunsplit

import pytest

from candidate import SignatureError, sign_url, verify_url

KEY = b"k" * 32


def edit_query(url, change):
    parts = urlsplit(url)
    params = change(parse_qsl(parts.query, keep_blank_values=True))
    return urlunsplit(parts._replace(query=urlencode(params)))


def test_round_trip_returns_original_params():
    url = sign_url("https://cdn.test/files/report.pdf?user=7&tag=", KEY, 1000)
    assert "expires=1000" in url
    assert verify_url(url, KEY, 999) == {"user": "7", "tag": ""}


def test_expiry_is_exclusive():
    url = sign_url("https://cdn.test/a", KEY, 1000)
    with pytest.raises(SignatureError, match="expired"):
        verify_url(url, KEY, 1000)


def test_parameter_order_does_not_matter():
    url = sign_url("https://cdn.test/a?b=2&a=1", KEY, 50)
    shuffled = edit_query(url, lambda ps: list(reversed(ps)))
    assert verify_url(shuffled, KEY, 1) == {"a": "1", "b": "2"}


def test_tampering_is_detected():
    url = sign_url("https://cdn.test/a?user=7", KEY, 50)
    forged = [
        edit_query(url, lambda ps: [(k, "8" if k == "user" else v) for k, v in ps]),
        edit_query(url, lambda ps: [(k, "99" if k == "expires" else v) for k, v in ps]),
        url.replace("/a?", "/b?"),
        edit_query(url, lambda ps: [p for p in ps if p[0] != "sig"]),
        edit_query(url, lambda ps: ps + [p for p in ps if p[0] == "sig"]),
    ]
    for candidate_url in forged:
        with pytest.raises(SignatureError):
            verify_url(candidate_url, KEY, 1)
    with pytest.raises(SignatureError):
        verify_url(url, b"other", 1)


def test_reserved_params_cannot_be_presigned():
    with pytest.raises(ValueError):
        sign_url("https://cdn.test/a?expires=5", KEY, 50)
    with pytest.raises(ValueError):
        sign_url("https://cdn.test/a?sig=x", KEY, 50)
