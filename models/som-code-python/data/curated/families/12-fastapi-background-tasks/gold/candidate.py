"""Accept orders at once and finish processing them in a background task."""

from typing import Any

from fastapi import BackgroundTasks, FastAPI, HTTPException, status
from pydantic import BaseModel, Field


class OrderRequest(BaseModel):
    """Payload accepted when submitting an order."""

    order_id: str = Field(min_length=1)
    customer_email: str = Field(min_length=3)
    item: str = Field(min_length=1)
    quantity: int = Field(gt=0)


ORDERS_DB: dict[str, dict[str, Any]] = {}
AUDIT_LOG: list[dict[str, Any]] = []


def reset_db() -> None:
    """Forget every order and audit entry."""
    ORDERS_DB.clear()
    AUDIT_LOG.clear()


def process_order(order_id: str) -> None:
    """Mark an order processed and append its audit entry."""
    order = ORDERS_DB.get(order_id)
    if order is not None:
        order["status"] = "processed"
        AUDIT_LOG.append(
            {
                "event": "order_processed",
                "order_id": order_id,
                "customer_email": order["customer_email"],
                "item": order["item"],
                "quantity": order["quantity"],
            }
        )


app = FastAPI(title="Order Background Processing Service")


@app.post("/orders", status_code=status.HTTP_202_ACCEPTED)
async def submit_order(
    order_req: OrderRequest, background_tasks: BackgroundTasks
) -> dict[str, str]:
    """Store a pending order, schedule its processing, and answer 202."""
    if order_req.order_id in ORDERS_DB:
        raise HTTPException(
            status_code=status.HTTP_409_CONFLICT,
            detail=f"Order '{order_req.order_id}' already exists",
        )

    normalized_email = order_req.customer_email.strip().lower()
    order_data = {
        "order_id": order_req.order_id,
        "customer_email": normalized_email,
        "item": order_req.item,
        "quantity": order_req.quantity,
        "status": "pending",
    }
    ORDERS_DB[order_req.order_id] = order_data

    background_tasks.add_task(process_order, order_req.order_id)

    return {
        "status": "accepted",
        "order_id": order_req.order_id,
        "order_status": "pending",
    }


@app.get("/orders/{order_id}")
async def get_order(order_id: str) -> dict[str, Any]:
    """Return one order by id, or 404 when it does not exist."""
    order = ORDERS_DB.get(order_id)
    if order is None:
        raise HTTPException(status_code=404, detail="Order not found")
    return order


@app.get("/audit-logs")
async def get_audit_logs() -> dict[str, list[dict[str, Any]]]:
    """Return a copy of the audit log."""
    return {"logs": list(AUDIT_LOG)}
