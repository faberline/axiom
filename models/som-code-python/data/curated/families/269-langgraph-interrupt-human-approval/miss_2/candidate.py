"""A LangGraph-style human-in-the-loop approval step using interrupt and resume."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any


class GraphInterrupt(Exception):  # noqa: N818
    """Raised by a node to pause the run and surface a value to a human."""

    def __init__(self, value: Any) -> None:
        super().__init__(value)
        self.value = value


@dataclass
class Pending:
    """A paused run waiting for a human decision."""

    request: dict[str, Any]
    question: str
    decisions: list[str] = field(default_factory=list)


class RefundWorkflow:
    """Pause large refunds for approval, then pay or reject on resume."""

    def __init__(self, pay: Callable[[str, int], None], threshold: int = 100) -> None:
        self._pay = pay
        self._threshold = threshold
        self._pending: dict[str, Pending] = {}

    def start(self, thread_id: str, order_id: str, amount: int) -> dict[str, Any]:
        """Run the workflow; large amounts return an interrupt payload."""
        if thread_id in self._pending:
            raise RuntimeError(f"thread {thread_id!r} is waiting for approval")
        if amount <= 0:
            raise ValueError("amount must be positive")
        request = {"order_id": order_id, "amount": amount}
        try:
            return self._review(request, None)
        except GraphInterrupt as pause:
            self._pending[thread_id] = Pending(request, str(pause.value))
            return {"status": "interrupted", "question": pause.value}

    def resume(self, thread_id: str, answer: str) -> dict[str, Any]:
        """Continue a paused thread with the human's answer."""
        pending = self._pending.pop(thread_id, None)
        if pending is None:
            raise KeyError(f"no pending approval for thread {thread_id!r}")
        if answer not in ("approve", "reject"):
            raise ValueError("answer must be approve or reject")
        pending.decisions.append(answer)
        return self._review(pending.request, answer)

    def _review(self, request: dict[str, Any], answer: str | None) -> dict[str, Any]:
        if request["amount"] >= self._threshold:
            if answer is None:
                raise GraphInterrupt(
                    f"Refund {request['amount']} for {request['order_id']}?"
                )
            if answer == "reject":
                return {"status": "rejected", **request}
        self._pay(request["order_id"], request["amount"])
        return {"status": "paid", **request}

    def is_waiting(self, thread_id: str) -> bool:
        """Return True while the thread is paused."""
        return thread_id in self._pending
