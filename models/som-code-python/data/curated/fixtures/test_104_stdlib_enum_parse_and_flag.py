import pytest

from candidate import OrderState, Permission, parse_permissions


def test_parse_ignores_case_and_spaces():
    assert OrderState.parse(" PAID ") is OrderState.PAID


def test_unknown_state_raises_value_error_with_cause():
    with pytest.raises(ValueError, match="unknown order state") as info:
        OrderState.parse("refunded")
    assert isinstance(info.value.__cause__, ValueError)


def test_transitions_follow_the_lifecycle():
    assert OrderState.PENDING.can_transition_to(OrderState.PAID)
    assert not OrderState.PAID.can_transition_to(OrderState.PENDING)
    assert not OrderState.SHIPPED.can_transition_to(OrderState.CANCELLED)


def test_permissions_combine_and_tolerate_trailing_commas():
    perms = parse_permissions("read, write,")
    assert perms == Permission.READ | Permission.WRITE
    assert Permission.DELETE not in perms
    assert Permission.DELETE in parse_permissions("admin")


def test_empty_spec_grants_nothing():
    assert parse_permissions("") == Permission(0)
    assert not parse_permissions("  ")


def test_unknown_permission_raises_value_error():
    with pytest.raises(ValueError, match="unknown permission: EXECUTE"):
        parse_permissions("read,execute")
