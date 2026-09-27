"""Stream an order export as CSV without building the whole file in memory."""

import csv
import io
from collections.abc import Iterable, Iterator
from typing import TypedDict

from fastapi import FastAPI
from fastapi.responses import StreamingResponse


class Order(TypedDict):
    """One exported order."""

    id: int
    customer: str
    total_cents: int
    status: str


ORDERS: list[Order] = [
    {"id": 1, "customer": "Ada, Countess", "total_cents": 1250, "status": "open"},
    {"id": 2, "customer": 'Bob "the builder"', "total_cents": 300, "status": "paid"},
    {"id": 3, "customer": "Cy", "total_cents": 99, "status": "open"},
]
HEADER = ("id", "customer", "total")

app = FastAPI(title="Order export")


def csv_rows(orders: Iterable[Order]) -> Iterator[str]:
    """Yield the header line, then one properly quoted line per order."""
    buffer = io.StringIO()
    writer = csv.writer(buffer)
    writer.writerow(HEADER)
    yield buffer.getvalue()
    for order in list(orders):
        buffer.seek(0)
        buffer.truncate()
        cents = order["total_cents"]
        total = f"{cents // 100}.{cents % 100:02d}"
        writer.writerow((order["id"], order["customer"], total))
        yield buffer.getvalue()


@app.get("/orders.csv")
def export_orders(status: str | None = None) -> StreamingResponse:
    """Stream matching orders as a CSV attachment."""
    selected = (o for o in ORDERS if status is None or o["status"] == status)
    return StreamingResponse(
        csv_rows(selected),
        media_type="text/csv",
        headers={"Content-Disposition": 'attachment; filename="orders.csv"'},
    )
