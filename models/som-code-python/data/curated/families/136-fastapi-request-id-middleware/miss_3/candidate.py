"""Tag every request with a safe correlation id and log how it finished."""

import re
import uuid
from collections.abc import Awaitable, Callable

from fastapi import FastAPI, Request, Response

SAFE_ID = re.compile(r"[A-Za-z0-9._-]{1,64}")
LOG: list[tuple[str, str, str, int]] = []

app = FastAPI(title="Traced")


def reset_db() -> None:
    """Clear the access log."""
    LOG.clear()


def request_id_for(incoming: str | None) -> str:
    """Keep a well-formed incoming id, otherwise mint a fresh one."""
    if incoming and SAFE_ID.fullmatch(incoming):
        return incoming
    return str(uuid.uuid4())


@app.middleware("http")
async def request_id(
    request: Request, call_next: Callable[[Request], Awaitable[Response]]
) -> Response:
    """Attach the id to request state, the response header and the log."""
    rid = request_id_for(request.headers.get("x-request-id"))
    request.state.request_id = rid
    status_code = 500
    try:
        response = await call_next(request)
        status_code = response.status_code
    finally:
        LOG.append((rid, request.method, request.url.path, status_code))
    response.headers["X-Request-Id"] = rid
    return response


@app.get("/whoami")
def whoami(request: Request) -> dict[str, str]:
    """Report the id the middleware assigned."""
    rid: str = request.state.request_id
    return {"request_id": rid}


@app.get("/boom")
def boom() -> None:
    """Fail on purpose."""
    raise RuntimeError("boom")
