"""Parse order states and permission flags from wire strings with explicit failures."""

from enum import Enum, Flag, auto
from typing import Self


class OrderState(Enum):
    """Lifecycle states of an order as they appear on the wire."""

    PENDING = "pending"
    PAID = "paid"
    SHIPPED = "shipped"
    CANCELLED = "cancelled"

    @classmethod
    def parse(cls, raw: str) -> Self:
        """Return the state for a wire value, ignoring case and surrounding spaces."""
        try:
            return cls(raw.strip().lower())
        except ValueError as exc:
            raise ValueError(f"unknown order state: {raw!r}") from exc

    def can_transition_to(self, target: "OrderState") -> bool:
        """Return whether the lifecycle allows moving from this state to target."""
        return target in TRANSITIONS[self]


class Permission(Flag):
    """Permissions that combine into one value."""

    READ = auto()
    WRITE = auto()
    DELETE = auto()
    ADMIN = READ | WRITE | DELETE


TRANSITIONS: dict[OrderState, frozenset[OrderState]] = {
    OrderState.PENDING: frozenset({OrderState.PAID, OrderState.CANCELLED}),
    OrderState.PAID: frozenset({OrderState.SHIPPED, OrderState.CANCELLED}),
    OrderState.SHIPPED: frozenset({OrderState.CANCELLED}),
    OrderState.CANCELLED: frozenset(),
}


def parse_permissions(spec: str) -> Permission:
    """Combine comma-separated permission names into one flag value."""
    result = Permission(0)
    for name in filter(None, (part.strip().upper() for part in spec.split(","))):
        if name not in Permission.__members__:
            raise ValueError(f"unknown permission: {name}")
        result |= Permission[name]
    return result
