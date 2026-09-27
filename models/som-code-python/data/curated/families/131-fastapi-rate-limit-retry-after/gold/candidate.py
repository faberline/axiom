"""Per-client fixed-window rate limiting that tells callers when to retry."""

import math
from typing import Annotated

from fastapi import Depends, FastAPI, Header, HTTPException, Response, status

LIMIT = 5
WINDOW = 60
NOW = [0.0]


def clock() -> float:
    """Return the current time; tests move NOW instead of sleeping."""
    return NOW[0]


class FixedWindow:
    """Count hits per key inside windows of WINDOW seconds."""

    def __init__(self, limit: int, window: int) -> None:
        if limit < 1 or window < 1:
            raise ValueError("limit and window must be positive")
        self.limit = limit
        self.window = window
        self.hits: dict[str, tuple[float, int]] = {}

    def hit(self, key: str, now: float) -> tuple[int, int]:
        """Record a hit and return (remaining, retry_after); retry 0 means allowed."""
        start, count = self.hits.get(key, (now, 0))
        if now - start >= self.window:
            start, count = now, 0
        if count >= self.limit:
            return 0, max(1, math.ceil(start + self.window - now))
        self.hits[key] = (start, count + 1)
        return self.limit - count - 1, 0


LIMITER = FixedWindow(LIMIT, WINDOW)


def reset_db() -> None:
    """Forget every client's window."""
    LIMITER.hits.clear()


def rate_limited(
    response: Response, x_client_id: Annotated[str | None, Header()] = None
) -> str:
    """Admit the request or raise 429 with Retry-After."""
    if not x_client_id:
        raise HTTPException(
            status.HTTP_400_BAD_REQUEST, detail="X-Client-Id header required"
        )
    remaining, retry_after = LIMITER.hit(x_client_id, clock())
    if retry_after:
        raise HTTPException(
            status.HTTP_429_TOO_MANY_REQUESTS,
            detail="rate limit exceeded",
            headers={"Retry-After": str(retry_after)},
        )
    response.headers["X-RateLimit-Remaining"] = str(remaining)
    return x_client_id


app = FastAPI(title="Search")


@app.get("/search")
def search(client_id: Annotated[str, Depends(rate_limited)]) -> dict[str, str]:
    """Answer an admitted search."""
    return {"client": client_id, "results": "none"}
