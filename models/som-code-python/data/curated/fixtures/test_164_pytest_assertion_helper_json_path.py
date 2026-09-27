import re

import pytest

from candidate import assert_subset

PAYLOAD = {
    "id": 7,
    "user": {"name": "ada", "roles": ["admin", "viewer"], "manager": None},
    "active": True,
}


def fails(message):
    return pytest.raises(AssertionError, match=re.escape(message))


def test_subset_with_extra_keys_passes():
    assert_subset(PAYLOAD, {"user": {"name": "ada"}})
    assert_subset(PAYLOAD, {"user": {"manager": None}, "active": True})


def test_mismatch_inside_a_list_reports_the_index_path():
    with fails("$.user.roles[1]: 'viewer' != 'admin'"):
        assert_subset(PAYLOAD, {"user": {"roles": ["admin", "admin"]}})


def test_missing_key_is_named():
    with fails("$.user: missing key 'email'"):
        assert_subset(PAYLOAD, {"user": {"email": "a@b"}})


@pytest.mark.parametrize(
    ("roles", "message"),
    [
        (["admin"], "$.user.roles: expected 1 items, got 2"),
        (["admin", "viewer", "owner"], "$.user.roles: expected 3 items, got 2"),
    ],
)
def test_list_lengths_must_match(roles, message):
    with fails(message):
        assert_subset(PAYLOAD, {"user": {"roles": roles}})


def test_types_must_match_exactly():
    with fails("$.active: True != 1"):
        assert_subset(PAYLOAD, {"active": 1})
    with fails("$.id: expected an object, got int"):
        assert_subset(PAYLOAD, {"id": {"value": 7}})


def test_helper_frames_are_hidden_from_pytest_tracebacks():
    assert "__tracebackhide__" in assert_subset.__code__.co_varnames
