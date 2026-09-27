"""Fixture for 181: property reads scripted with PropertyMock."""

import pytest

from candidate import average, decide, fake_sensor


@pytest.mark.parametrize(
    ("reading", "action"),
    [(19.4, "heat"), (19.5, "idle"), (20.0, "idle"), (20.5, "idle"), (20.6, "cool")],
)
def test_dead_band_around_the_target(reading: float, action: str) -> None:
    sensor, _ = fake_sensor(reading)
    assert decide(sensor, 20.0) == action


def test_decide_reads_the_property_once() -> None:
    sensor, prop = fake_sensor(20.0, 22.0)
    assert decide(sensor, 20.0) == "idle"
    prop.assert_called_once_with()


def test_average_consumes_one_reading_per_sample() -> None:
    sensor, prop = fake_sensor(19.0, 20.0, 24.0)
    assert average(sensor, 3) == pytest.approx(21.0)
    assert prop.call_count == 3


def test_band_must_be_positive() -> None:
    sensor, prop = fake_sensor(20.0)
    with pytest.raises(ValueError, match="band must be positive"):
        decide(sensor, 20.0, band=0)
    prop.assert_not_called()


def test_samples_must_be_positive() -> None:
    sensor, _ = fake_sensor(20.0)
    with pytest.raises(ValueError, match="samples"):
        average(sensor, 0)


def test_fake_sensors_are_independent() -> None:
    first, _ = fake_sensor(1.0)
    second, _ = fake_sensor(2.0)
    assert second.celsius == 2.0
    assert first.celsius == 1.0


def test_fake_sensor_needs_readings() -> None:
    with pytest.raises(ValueError, match="at least one reading"):
        fake_sensor()
