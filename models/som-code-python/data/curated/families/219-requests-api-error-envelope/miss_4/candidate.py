"""Turn HTTP error responses from a JSON API into typed exceptions."""

from __future__ import annotations

import requests

MAX_MESSAGE_CHARS = 200


class ApiError(Exception):
    """An error response with its status, machine code and message."""

    def __init__(self, status: int, code: str, message: str) -> None:
        super().__init__(f"{status} {code}: {message}")
        self.status = status
        self.code = code
        self.message = message


class NotFoundError(ApiError):
    """The requested resource does not exist."""


def raise_for_api_error(resp: requests.Response) -> None:
    """Raise ApiError for 4xx/5xx using the ``{"error": {...}}`` envelope."""
    if resp.ok:
        return
    try:
        body = resp.json()
    except requests.JSONDecodeError:
        body = None
    error = body.get("error") if isinstance(body, dict) else None
    if isinstance(error, dict):
        code = str(error.get("code", "unknown"))
        message = str(error.get("message", ""))
    else:
        code = "unknown"
        message = resp.text[:MAX_MESSAGE_CHARS]
    raise ApiError(resp.status_code, code, message)
