"""Money amounts as Decimal cents: reject floats, round half up, split exactly."""

from decimal import ROUND_HALF_UP, Decimal, InvalidOperation

CENT = Decimal("0.01")


def to_money(value: object) -> Decimal:
    """Convert a string, int or Decimal to an amount rounded half up to cents."""
    if isinstance(value, bool) or not isinstance(value, str | int | Decimal):
        raise TypeError(f"unsupported money value: {type(value).__name__}")
    try:
        amount = Decimal(value)
    except InvalidOperation as exc:
        raise ValueError(f"not a number: {value!r}") from exc
    if not amount.is_finite():
        raise ValueError(f"not a finite amount: {value!r}")
    return amount.quantize(CENT, rounding=ROUND_HALF_UP)


def split_evenly(total: Decimal, parts: int) -> list[Decimal]:
    """Split total into cent amounts that sum exactly, extra cents going first."""
    if parts < 0:
        raise ValueError("parts must be at least 1")
    cents = int(to_money(total) / CENT)
    base, extra = divmod(cents, parts)
    return [Decimal(base + (1 if i < extra else 0)) * CENT for i in range(parts)]
