import json

import pytest
from pydantic import ValidationError

from candidate import ApiResponseError, User, fetch_user, fetch_users


def body(**fields):
    return json.dumps({"request_id": "req-0001", **fields})


def test_single_user_is_validated_as_a_model():
    user = fetch_user(body(data={"id": "7", "name": "Ada"}))
    assert isinstance(user, User)
    assert user.id == 7


def test_list_payload_uses_the_same_envelope():
    users = fetch_users(body(data=[{"id": 1, "name": "A"}, {"id": 2, "name": "B"}]))
    assert [u.name for u in users] == ["A", "B"]
    assert all(isinstance(u, User) for u in users)


@pytest.mark.parametrize("data", [{"id": 0, "name": "Z"}, {"name": "no id"}])
def test_payload_is_validated_against_the_type_parameter(data):
    with pytest.raises(ValidationError):
        fetch_user(body(data=data))


def test_errors_raise_even_with_data():
    raw = body(
        data={"id": 1, "name": "A"},
        errors=[
            {"code": "not_found", "message": "no such user"},
            {"code": "stale", "message": "cache expired"},
        ],
    )
    with pytest.raises(ApiResponseError) as info:
        fetch_user(raw)
    assert str(info.value) == "req-0001: not_found: no such user; stale: cache expired"
    assert [e.code for e in info.value.errors] == ["not_found", "stale"]


def test_empty_response_is_a_value_error():
    with pytest.raises(ValueError, match="req-0001: empty response"):
        fetch_user(body())


def test_request_id_must_be_at_least_eight_characters():
    with pytest.raises(ValidationError):
        fetch_user(json.dumps({"request_id": "req-1", "data": {"id": 1, "name": "A"}}))
