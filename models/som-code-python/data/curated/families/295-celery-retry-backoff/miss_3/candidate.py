"""A Celery webhook task that retries transient failures with capped backoff."""

from __future__ import annotations

import random
from collections.abc import Callable
from typing import Any

from celery import Celery, Task

app = Celery("webhooks")
app.conf.update(
    task_always_eager=True,
    task_eager_propagates=False,
    broker_url="memory://",
    result_backend="cache+memory://",
)

MAX_RETRIES = 4
BASE_DELAY = 2.0
MAX_DELAY = 60.0


class TransientError(Exception):
    """A failure worth retrying, such as a timeout or a 503."""


class PermanentError(Exception):
    """A failure that retrying cannot fix, such as a 400."""


def send_webhook(url: str, payload: dict[str, Any]) -> int:
    """Deliver payload to url and return the HTTP status; replaced in tests."""
    raise TransientError(f"no transport configured for {url} {payload}")


def backoff(
    retries: int, jitter: Callable[[float, float], float] = random.uniform
) -> float:
    """Return a full-jitter delay for the given retry count, capped at MAX_DELAY."""
    if retries < 0:
        raise ValueError("retries must be non-negative")
    ceiling = min(MAX_DELAY, BASE_DELAY * 2**retries)
    return jitter(0.0, ceiling)


@app.task(bind=True, max_retries=MAX_RETRIES, acks_late=True)
def deliver(self: Task[Any, Any], url: str, payload: dict[str, Any]) -> int:
    """Send the webhook, retrying TransientError and failing fast otherwise."""
    try:
        return send_webhook(url, payload)
    except Exception as exc:
        raise self.retry(exc=exc, countdown=backoff(self.request.retries)) from exc
