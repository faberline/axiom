import pytest

from candidate import Bounded, Thermostat


def test_each_instance_keeps_its_own_values():
    living = Thermostat(21)
    bedroom = Thermostat(17, humidity=55)
    assert (living.target, living.humidity) == (21, 40)
    assert (bedroom.target, bedroom.humidity) == (17, 55)


def test_assignment_is_checked_after_construction():
    t = Thermostat(20)
    t.target = 25
    assert t.target == 25
    with pytest.raises(ValueError, match="target must be between 5 and 30"):
        t.target = 31
    assert t.target == 25


def test_bounds_are_inclusive():
    assert Thermostat(5).target == 5
    assert Thermostat(30, humidity=100).humidity == 100
    with pytest.raises(ValueError, match="humidity must be between 0 and 100"):
        Thermostat(20, humidity=100.5)


def test_class_access_returns_the_descriptor():
    assert isinstance(Thermostat.target, Bounded)
    assert (Thermostat.target.low, Thermostat.target.high) == (5, 30)


def test_inverted_bounds_are_rejected():
    with pytest.raises(ValueError, match="low must not exceed high"):
        Bounded(10, 1)
