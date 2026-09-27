import pytest

from candidate import Status, advance


def test_lookup_ignores_case_and_whitespace():
    assert Status(" PAID ") is Status.PAID
    assert Status("pending") is Status.PENDING


def test_legacy_spellings_map_to_current_members():
    assert Status("canceled") is Status.CANCELLED
    assert Status("Sent") is Status.SHIPPED


@pytest.mark.parametrize("value", ["lost", 3, None, ""])
def test_unknown_values_raise_value_error(value):
    with pytest.raises(ValueError, match="is not a valid Status"):
        Status(value)


def test_final_states():
    assert [s for s in Status if s.is_final] == [Status.SHIPPED, Status.CANCELLED]


def test_allowed_moves():
    assert advance(Status.PENDING, "paid") is Status.PAID
    assert advance(Status.PAID, "SHIPPED") is Status.SHIPPED
    assert advance(Status.PENDING, Status.CANCELLED) is Status.CANCELLED
    assert advance(Status.PAID, "canceled") is Status.CANCELLED


def test_forbidden_moves():
    with pytest.raises(ValueError, match="cannot move from shipped to cancelled"):
        advance(Status.SHIPPED, "cancelled")
    with pytest.raises(ValueError, match="cannot move from pending to shipped"):
        advance(Status.PENDING, "shipped")
