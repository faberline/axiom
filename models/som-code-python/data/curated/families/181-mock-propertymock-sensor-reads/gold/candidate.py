"""Script property reads with a PropertyMock attached to the mock's own type."""

from typing import Protocol
from unittest.mock import MagicMock, PropertyMock


class Sensor(Protocol):
    """A thermometer exposing its reading as a property."""

    @property
    def celsius(self) -> float:
        """The current temperature in degrees Celsius."""


def fake_sensor(*readings: float) -> tuple[MagicMock, PropertyMock]:
    """A sensor whose celsius property returns readings in order, and that property."""
    if not readings:
        raise ValueError("fake_sensor needs at least one reading")
    sensor = MagicMock()
    prop = PropertyMock(side_effect=list(readings))
    type(sensor).celsius = prop
    return sensor, prop


def decide(sensor: Sensor, target: float, band: float = 0.5) -> str:
    """Return heat, cool or idle from a single reading and a dead band around target."""
    if band <= 0:
        raise ValueError("band must be positive")
    reading = sensor.celsius
    if reading < target - band:
        return "heat"
    if reading > target + band:
        return "cool"
    return "idle"


def average(sensor: Sensor, samples: int) -> float:
    """Mean of samples consecutive readings."""
    if samples < 1:
        raise ValueError("samples must be at least 1")
    return sum(sensor.celsius for _ in range(samples)) / samples
