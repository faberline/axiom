import asyncio
import socket

import aiohttp
import pytest
from aiohttp import web
from aiohttp.test_utils import TestServer

from candidate import FetchResult, fetch_all


def run(make_urls, **kwargs):
    state = {"now": 0, "max": 0}

    async def handler(request):
        state["now"] += 1
        state["max"] = max(state["max"], state["now"])
        await asyncio.sleep(float(request.query.get("d", "0.05")))
        state["now"] -= 1
        status = int(request.query.get("s", "200"))
        return web.Response(status=status, text=request.match_info["name"])

    async def main():
        app = web.Application()
        app.router.add_get("/{name}", handler)
        async with TestServer(app) as server, aiohttp.ClientSession() as session:
            urls = make_urls(lambda path: str(server.make_url(path)))
            return await fetch_all(session, urls, **kwargs)

    results = asyncio.run(main())
    return results, state["max"]


def test_limit_caps_requests_in_flight():
    results, peak = run(lambda u: [u(f"/n{i}") for i in range(12)], limit=3)
    assert peak == 3
    assert [r.body for r in results] == [f"n{i}".encode() for i in range(12)]


def test_default_limit_is_five():
    _, peak = run(lambda u: [u(f"/n{i}") for i in range(10)])
    assert peak == 5


def test_results_keep_input_order_not_completion_order():
    results, _ = run(lambda u: [u("/slow?d=0.2"), u("/fast?d=0")], limit=2)
    assert [r.body for r in results] == [b"slow", b"fast"]
    assert results[0].url.endswith("/slow?d=0.2")


def test_errors_and_statuses_are_reported_per_url():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    dead = f"http://127.0.0.1:{port}/x"
    results, _ = run(lambda u: [u("/ok"), dead, u("/gone?s=404")], limit=2)
    assert results[0] == FetchResult(results[0].url, 200, b"ok", None)
    assert results[1] == FetchResult(dead, None, None, "ClientConnectorError")
    assert results[2].status == 404
    assert results[2].error is None


def test_limit_must_be_positive():
    with pytest.raises(ValueError, match="at least 1"):
        run(lambda u: [u("/a")], limit=0)
