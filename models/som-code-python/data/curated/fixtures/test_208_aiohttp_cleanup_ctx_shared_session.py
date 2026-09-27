import asyncio
import socket

import aiohttp
from aiohttp import web
from aiohttp.test_utils import TestServer

from candidate import SESSION_KEY, make_app

TEMPS = {"oslo": 3.5, "cairo": 31.0}


def upstream_app(seen):
    async def forecast(request):
        city = request.query.get("city")
        seen.append((city, request.transport.get_extra_info("peername")[1]))
        if city == "busy":
            return web.json_response({"error": "slow down"}, status=429)
        if city not in TEMPS:
            return web.json_response({"error": "no such city"}, status=404)
        return web.json_response({"temp_c": TEMPS[city]})

    app = web.Application()
    app.router.add_get("/forecast", forecast)
    return app


def refused_url():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    return f"http://127.0.0.1:{port}"


def proxy(cities, upstream_url=None):
    seen = []

    async def main():
        async with TestServer(upstream_app(seen)) as upstream:
            base = upstream_url or str(upstream.make_url(""))
            app = make_app(base)
            results = []
            async with TestServer(app) as server, aiohttp.ClientSession() as client:
                for city in cities:
                    url = str(server.make_url(f"/weather/{city}"))
                    async with client.get(url) as resp:
                        results.append((resp.status, await resp.text()))
                session = app[SESSION_KEY]
                open_during_serving = not session.closed
            return results, session, open_during_serving

    results, session, was_open = asyncio.run(main())
    return results, seen, session, was_open


def test_forecast_is_proxied_with_the_city_param():
    results, seen, _, _ = proxy(["oslo"])
    assert results[0][0] == 200
    assert results[0][1] == '{"city": "oslo", "temp_c": 3.5}'
    assert [city for city, _ in seen] == ["oslo"]


def test_one_session_is_shared_and_closed_on_cleanup():
    results, seen, session, was_open = proxy(["oslo", "cairo", "oslo"])
    assert [status for status, _ in results] == [200, 200, 200]
    assert len({port for _, port in seen}) == 1
    assert was_open
    assert session.closed


def test_unknown_city_is_404():
    results, _, _, _ = proxy(["atlantis"])
    assert results[0][0] == 404
    assert "unknown city atlantis" in results[0][1]


def test_other_upstream_errors_are_502():
    results, _, _, _ = proxy(["busy"])
    assert results[0][0] == 502
    assert "upstream error" in results[0][1]


def test_unreachable_upstream_is_502():
    results, seen, _, _ = proxy(["oslo"], upstream_url=refused_url())
    assert results[0][0] == 502
    assert "upstream unavailable" in results[0][1]
    assert seen == []
