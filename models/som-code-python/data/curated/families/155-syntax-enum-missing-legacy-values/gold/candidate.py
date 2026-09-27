"""Order statuses as an Enum that accepts legacy spellings and checks moves."""

from enum import Enum, unique

_LEGACY = {"canceled": "cancelled", "sent": "shipped"}


@unique
class Status(Enum):
    """The lifecycle states of an order."""

    PENDING = "pending"
    PAID = "paid"
    SHIPPED = "shipped"
    CANCELLED = "cancelled"

    @classmethod
    def _missing_(cls, value: object) -> "Status | None":
        if not isinstance(value, str):
            return None
        key = value.strip().lower()
        key = _LEGACY.get(key, key)
        for member in cls:
            if member.value == key:
                return member
        return None

    @property
    def is_final(self) -> bool:
        """Whether no further transition is possible."""
        return self in {Status.SHIPPED, Status.CANCELLED}


TRANSITIONS: dict[Status, frozenset[Status]] = {
    Status.PENDING: frozenset({Status.PAID, Status.CANCELLED}),
    Status.PAID: frozenset({Status.SHIPPED, Status.CANCELLED}),
    Status.SHIPPED: frozenset(),
    Status.CANCELLED: frozenset(),
}


def advance(current: Status, target: str | Status) -> Status:
    """Return the new status, refusing values and moves that are not allowed."""
    new = Status(target)
    if new not in TRANSITIONS[current]:
        raise ValueError(f"cannot move from {current.value} to {new.value}")
    return new
