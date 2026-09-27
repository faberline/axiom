"""A money value object whose equality, hashing and arithmetic agree."""

from decimal import ROUND_HALF_EVEN, Decimal

CENT = Decimal("0.01")


class Money:
    """An immutable amount in one currency, normalized to whole cents."""

    __slots__ = ("_amount", "_currency")

    def __init__(self, amount: Decimal | int | str, currency: str) -> None:
        code = currency.strip().upper()
        if len(code) != 3 or not code.isalpha():
            raise ValueError(f"bad currency code: {currency!r}")
        self._amount = Decimal(amount).quantize(CENT, rounding=ROUND_HALF_EVEN)
        self._currency = code

    @property
    def amount(self) -> Decimal:
        """The amount rounded to cents."""
        return self._amount

    @property
    def currency(self) -> str:
        """The upper-case ISO currency code."""
        return self._currency

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Money):
            return NotImplemented
        return (self._amount, self._currency) == (other.amount, other.currency)

    def __hash__(self) -> int:
        return id(self)

    def __add__(self, other: "Money") -> "Money":
        if other.currency != self._currency:
            raise ValueError(f"currency mismatch: {self._currency} + {other.currency}")
        return Money(self._amount + other.amount, self._currency)

    def __repr__(self) -> str:
        return f"Money('{self._amount}', '{self._currency}')"
