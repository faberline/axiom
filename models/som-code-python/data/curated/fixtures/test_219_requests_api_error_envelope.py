import json

import pytest
import requests

from candidate import ApiError, NotFoundError, raise_for_api_error


def response(status, body):
    resp = requests.Response()
    resp.status_code = status
    resp.encoding = "utf-8"
    raw = body if isinstance(body, bytes) else json.dumps(body).encode()
    resp._content = raw
    return resp


def test_success_and_redirect_statuses_do_not_raise():
    raise_for_api_error(response(200, {"ok": True}))
    raise_for_api_error(response(201, {"id": 1}))
    raise_for_api_error(response(304, b""))


def test_envelope_fields_are_used():
    body = {"error": {"code": "invalid_field", "message": "name is required"}}
    with pytest.raises(ApiError) as info:
        raise_for_api_error(response(422, body))
    err = info.value
    assert (err.status, err.code, err.message) == (
        422,
        "invalid_field",
        "name is required",
    )
    assert type(err) is ApiError


def test_404_raises_not_found():
    body = {"error": {"code": "no_such_item", "message": "item 7"}}
    with pytest.raises(NotFoundError) as info:
        raise_for_api_error(response(404, body))
    assert info.value.code == "no_such_item"


def test_non_json_body_is_truncated_text():
    html = b"<html>" + b"x" * 500 + b"</html>"
    with pytest.raises(ApiError) as info:
        raise_for_api_error(response(502, html))
    assert info.value.code == "unknown"
    assert info.value.message == html.decode()[:200]


def test_malformed_envelopes_fall_back_to_text():
    with pytest.raises(ApiError) as info:
        raise_for_api_error(response(500, {"error": "boom"}))
    assert info.value.code == "unknown"
    assert info.value.message == '{"error": "boom"}'
    with pytest.raises(ApiError) as info:
        raise_for_api_error(response(400, {"error": {"code": "bad"}}))
    assert (info.value.code, info.value.message) == ("bad", "")
