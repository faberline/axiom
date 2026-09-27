import pytest

from candidate import FieldError, import_users, validate_user

GOOD = {"name": "Ada", "age": 36, "email": "ada@example.com"}


def test_valid_record_passes():
    assert validate_user(GOOD) is None
    assert validate_user({**GOOD, "age": 150}) is None
    assert validate_user({**GOOD, "age": 0}) is None


def test_every_invalid_field_is_reported_together():
    with pytest.raises(ExceptionGroup, match="invalid user") as info:
        validate_user({"name": "", "age": "x", "email": "nope"})
    errors = info.value.exceptions
    assert all(isinstance(e, FieldError) for e in errors)
    assert sorted(e.field for e in errors) == ["age", "email", "name"]


@pytest.mark.parametrize("age", [151, -1, True, 36.0, None])
def test_bad_ages(age):
    with pytest.raises(ExceptionGroup) as info:
        validate_user({**GOOD, "age": age})
    assert [e.field for e in info.value.exceptions] == ["age"]


def test_blank_name_is_required():
    with pytest.raises(ExceptionGroup) as info:
        validate_user({**GOOD, "name": "   "})
    assert str(info.value.exceptions[0]) == "name: required"


def test_import_collects_failures_by_index():
    records = [
        GOOD,
        {**GOOD, "age": 200},
        {**GOOD, "name": "Grace"},
        {"age": 5, "email": "kid"},
    ]
    assert import_users(records) == (2, {1: ["age"], 3: ["email", "name"]})
