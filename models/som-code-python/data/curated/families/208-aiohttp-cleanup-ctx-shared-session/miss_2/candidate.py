"""Share one upstream ClientSession across aiohttp.web handlers."""

from __future__ import annotations

from collections.abc import AsyncIterator

import aiohttp
from aiohttp import web

UPSTREAM_KEY = web.AppKey("upstream_url", str)
SESSION_KEY = web.AppKey("session", aiohttp.ClientSession)


async def client_session_ctx(app: web.Application) -> AsyncIterator[None]:
    """Open the shared upstream session on startup and close it on cleanup."""
    session = aiohttp.ClientSession(base_url=app[UPSTREAM_KEY])
    app[SESSION_KEY] = session
    yield


async def weather(request: web.Request) -> web.Response:
    """Proxy the upstream forecast for one city."""
    city = request.match_info["city"]
    session = request.app[SESSION_KEY]
    try:
        async with session.get("/forecast", params={"city": city}) as resp:
            if resp.status == 404:
                raise web.HTTPNotFound(reason=f"unknown city {city}")
            if resp.status != 200:
                raise web.HTTPBadGateway(reason="upstream error")
            data = await resp.json()
    except aiohttp.ClientConnectionError as exc:
        raise web.HTTPBadGateway(reason="upstream unavailable") from exc
    return web.json_response({"city": city, "temp_c": data["temp_c"]})


def make_app(upstream_url: str) -> web.Application:
    """Build the proxy application for ``upstream_url``."""
    app = web.Application()
    app[UPSTREAM_KEY] = upstream_url
    app.cleanup_ctx.append(client_session_ctx)
    app.router.add_get("/weather/{city}", weather)
    return app
