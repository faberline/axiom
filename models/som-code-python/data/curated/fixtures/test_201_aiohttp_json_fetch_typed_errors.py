import asyncio

import aiohttp
import pytest
from aiohttp import web
from aiohttp.test_utils import TestServer

from candidate import ApiError, fetch_json


async def item(request):
    return web.json_response(
        {"id": request.query.get("id"), "accept": request.headers["Accept"]}
    )


async def bad(request):
    return web.Response(status=400, text="bad input")


async def boom(request):
    return web.Response(status=503, text="x" * 300)


async def html(request):
    return web.Response(text="<p>hi</p>", content_type="text/html")


async def listing(request):
    return web.json_response([1, 2])


def run(check):
    async def main():
        app = web.Application()
        for path, handler in [
            ("/item", item),
            ("/bad", bad),
            ("/boom", boom),
            ("/html", html),
            ("/list", listing),
        ]:
            app.router.add_get(path, handler)
        async with TestServer(app) as server, aiohttp.ClientSession() as session:
            await check(session, lambda path: str(server.make_url(path)))

    asyncio.run(main())


def test_returns_the_object_and_sends_params_and_accept():
    async def check(session, url):
        data = await fetch_json(session, url("/item"), params={"id": "7"})
        assert data == {"id": "7", "accept": "application/json"}

    run(check)


def test_client_error_carries_status_and_body():
    async def check(session, url):
        with pytest.raises(ApiError) as info:
            await fetch_json(session, url("/bad"))
        assert info.value.status == 400
        assert info.value.message == "bad input"

    run(check)


def test_server_error_body_is_truncated_to_200_chars():
    async def check(session, url):
        with pytest.raises(ApiError) as info:
            await fetch_json(session, url("/boom"))
        assert info.value.status == 503
        assert info.value.message == "x" * 200
        assert str(info.value).startswith("503: x")

    run(check)


def test_non_json_content_type_is_an_api_error():
    async def check(session, url):
        with pytest.raises(ApiError, match="expected JSON, got text/html"):
            await fetch_json(session, url("/html"))

    run(check)


def test_json_array_is_an_api_error():
    async def check(session, url):
        with pytest.raises(ApiError, match="JSON object"):
            await fetch_json(session, url("/list"))

    run(check)
