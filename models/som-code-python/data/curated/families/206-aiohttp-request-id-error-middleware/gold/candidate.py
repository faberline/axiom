"""Request-id propagation and JSON error bodies as aiohttp.web middleware."""

from __future__ import annotations

import logging
import re
import uuid
from collections.abc import Awaitable, Callable

from aiohttp import web

REQUEST_ID_HEADER = "X-Request-ID"
_VALID_ID = re.compile(r"[A-Za-z0-9-]{1,64}")

Handler = Callable[[web.Request], Awaitable[web.StreamResponse]]
logger = logging.getLogger(__name__)


@web.middleware
async def request_id_middleware(
    request: web.Request, handler: Handler
) -> web.StreamResponse:
    """Tag every request with an id and turn errors into JSON responses."""
    incoming = request.headers.get(REQUEST_ID_HEADER, "")
    request_id = incoming if _VALID_ID.fullmatch(incoming) else uuid.uuid4().hex
    request["request_id"] = request_id
    response: web.StreamResponse
    try:
        response = await handler(request)
    except web.HTTPException as exc:
        body = {"error": exc.reason, "request_id": request_id}
        response = web.json_response(body, status=exc.status)
    except Exception:  # pylint: disable=broad-exception-caught
        logger.exception("unhandled error in request %s", request_id)
        body = {"error": "internal server error", "request_id": request_id}
        response = web.json_response(body, status=500)
    response.headers[REQUEST_ID_HEADER] = request_id
    return response
