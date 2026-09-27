import pytest
from pydantic import ValidationError

from candidate import Profile, apply_update


@pytest.fixture
def profile():
    return Profile(handle="ada", display_name="Ada")


def test_update_returns_a_new_version(profile):
    updated = apply_update(profile, {"display_name": "Ada L."})
    assert updated.display_name == "Ada L."
    assert updated.version == 2
    assert profile.display_name == "Ada"
    assert profile.version == 1


def test_updates_are_validated(profile):
    with pytest.raises(ValidationError):
        apply_update(profile, {"display_name": ""})
    with pytest.raises(ValidationError):
        apply_update(profile, {"display_name": "x" * 51})


def test_updates_are_coerced_like_construction(profile):
    updated = apply_update(profile, {"tags": ["python", "math"]})
    assert updated.tags == ("python", "math")


def test_non_editable_fields_are_refused(profile):
    with pytest.raises(ValueError, match="fields not editable: handle, version"):
        apply_update(profile, {"version": 9, "handle": "eve", "tags": []})


def test_empty_update_keeps_the_same_object(profile):
    assert apply_update(profile, {}) is profile


def test_profiles_are_frozen(profile):
    with pytest.raises(ValidationError):
        profile.display_name = "Mallory"
