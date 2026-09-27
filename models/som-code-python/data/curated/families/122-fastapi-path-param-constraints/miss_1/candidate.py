"""Sales report API that validates path parameters before touching the data."""

from enum import StrEnum
from typing import Annotated

from fastapi import FastAPI, HTTPException, Path, Query, status


class Region(StrEnum):
    """Sales regions accepted in the URL."""

    EU = "eu"
    US = "us"
    APAC = "apac"


SALES: dict[tuple[Region, int], list[int]] = {
    (Region.EU, 2025): [120, 80, 95, 110, 130, 150, 170, 160, 140, 125, 115, 210],
    (Region.US, 2025): [300, 280, 310, 330, 350, 360, 390, 370, 340, 320, 310, 450],
}
app = FastAPI(title="Sales reports")


@app.get("/sales/{region}/{year}/{month}")
def month_total(
    region: str,
    year: Annotated[int, Path(ge=2000, le=2100)],
    month: Annotated[int, Path(ge=1, le=12)],
    scale: Annotated[int, Query(ge=1, le=1000)] = 1,
) -> dict[str, str | int]:
    """Return one month's sales total for a region, divided by scale."""
    data = SALES.get((region, year))
    if data is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, f"no sales for {region} {year}")
    return {
        "region": region,
        "year": year,
        "month": month,
        "total": data[month - 1] // scale,
    }
