"""Validate a frozen parcel record at construction and derive its billable weight."""

from dataclasses import dataclass, field

VOLUMETRIC_DIVISOR = 6000
MAX_WEIGHT_KG = 70.0


@dataclass(frozen=True, slots=True)
class Parcel:
    """An immutable parcel whose billable weight is fixed at construction."""

    weight_kg: float
    length_cm: int
    width_cm: int
    height_cm: int
    tags: list[str] = field(default_factory=list)
    billable_kg: float = field(init=False)

    def __post_init__(self) -> None:
        if not 0 < self.weight_kg <= MAX_WEIGHT_KG:
            raise ValueError(f"weight must be in (0, {MAX_WEIGHT_KG}] kg")
        if min(self.length_cm, self.width_cm, self.height_cm) <= 0:
            raise ValueError("dimensions must be positive")
        volume = self.length_cm * self.width_cm * self.height_cm
        volumetric = volume / VOLUMETRIC_DIVISOR
        billable = round(max(self.weight_kg, volumetric), 2)
        object.__setattr__(self, "billable_kg", billable)
