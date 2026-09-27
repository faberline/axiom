"""Range-checked attributes through a descriptor that learns its own name."""

from typing import Self, overload


class Bounded:
    """A numeric attribute that must stay within low and high, inclusive."""

    def __init__(self, low: float, high: float) -> None:
        if low > high:
            raise ValueError("low must not exceed high")
        self.low = low
        self.high = high
        self.name = ""
        self.slot = ""

    def __set_name__(self, owner: type, name: str) -> None:
        self.name = name
        self.slot = f"_{name}"

    @overload
    def __get__(self, instance: None, owner: type) -> Self: ...

    @overload
    def __get__(self, instance: object, owner: type) -> float: ...

    def __get__(self, instance: object | None, owner: type) -> Self | float:
        if instance is None:
            return self
        value: float = getattr(instance, self.slot)
        return value

    def __set__(self, instance: object, value: float) -> None:
        if not self.low <= value <= self.high:
            raise ValueError(f"{self.name} must be between {self.low} and {self.high}")
        setattr(instance, self.slot, value)


class Thermostat:
    """A thermostat whose settings are range-checked on every assignment."""

    target = Bounded(5, 30)
    humidity = Bounded(0, 100)

    def __init__(self, target: float, humidity: float = 40) -> None:
        self.target = target
        self.humidity = humidity
