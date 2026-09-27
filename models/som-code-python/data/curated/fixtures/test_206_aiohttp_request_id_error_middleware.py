import asyncio
import logging
import re

import aiohttp
from aiohttp import web
from aiohttp.test_utils import TestServer

from candidate import request_id_middleware

HEX_ID = re.compile(r"[0-9a-f]{32}")


async def ok(request):
    return web.json_response({"seen": request["request_id"]})


async def boom(request):
    raise RuntimeError("db password is hunter2")


async def forbidden(request):
    raise web.HTTPForbidden(reason="no access")


def call(path, headers=None):
    async def main():
        app = web.Application(middlewares=[request_id_middleware])
        app.router.add_get("/ok", ok)
        app.router.add_get("/boom", boom)
        app.router.add_get("/forbidden", forbidden)
        async with TestServer(app) as server, aiohttp.ClientSession() as session:
            url = str(server.make_url(path))
            async with session.get(url, headers=headers or {}) as resp:
                header = resp.headers.get("X-Request-ID")
                return resp.status, await resp.json(), header

    return asyncio.run(main())


def test_valid_incoming_id_is_kept():
    long_id = "a" * 64
    assert call("/ok", {"X-Request-ID": "abc-123"}) == (
        200,
        {"seen": "abc-123"},
        "abc-123",
    )
    assert call("/ok", {"X-Request-ID": long_id}) == (200, {"seen": long_id}, long_id)


def test_invalid_or_missing_ids_are_generated():
    for headers in ({"X-Request-ID": "bad id!"}, {"X-Request-ID": "a" * 65}, None):
        status, body, header = call("/ok", headers)
        assert status == 200
        assert HEX_ID.fullmatch(header)
        assert body == {"seen": header}
    assert call("/ok")[2] != call("/ok")[2]


def test_unhandled_error_is_generic_json_500(caplog):
    with caplog.at_level(logging.ERROR):
        status, body, header = call("/boom", {"X-Request-ID": "req-1"})
    assert status == 500
    assert body == {"error": "internal server error", "request_id": "req-1"}
    assert header == "req-1"
    assert "hunter2" not in str(body)
    assert any("req-1" in record.getMessage() for record in caplog.records)


def test_http_exceptions_keep_their_status():
    assert call("/forbidden", {"X-Request-ID": "req-2"}) == (
        403,
        {"error": "no access", "request_id": "req-2"},
        "req-2",
    )
    status, body, header = call("/missing")
    assert status == 404
    assert body == {"error": "Not Found", "request_id": header}
