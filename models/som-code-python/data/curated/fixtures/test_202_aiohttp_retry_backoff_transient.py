import asyncio
import socket

import aiohttp
import pytest
from aiohttp import web
from aiohttp.test_utils import TestServer

from candidate import get_with_retry


def scripted(*replies):
    calls = []

    async def handler(request):
        calls.append(1)
        status, headers = replies[min(len(calls), len(replies)) - 1]
        return web.Response(status=status, headers=headers, text=f"r{len(calls)}")

    return handler, calls


def run(replies, attempts=4):
    handler, calls = scripted(*replies)
    sleeps = []

    async def fake_sleep(seconds):
        sleeps.append(seconds)

    async def main():
        app = web.Application()
        app.router.add_get("/", handler)
        async with TestServer(app) as server, aiohttp.ClientSession() as session:
            url = str(server.make_url("/"))
            return await get_with_retry(
                session, url, attempts=attempts, sleep=fake_sleep
            )

    return lambda: asyncio.run(main()), calls, sleeps


def test_retries_transient_statuses_with_exponential_backoff():
    call, calls, sleeps = run([(503, {}), (502, {}), (200, {})])
    assert call() == b"r3"
    assert len(calls) == 3
    assert sleeps == [0.5, 1.0]


def test_retry_after_is_honoured_and_capped():
    call, _, sleeps = run([(429, {"Retry-After": "3"}), (200, {})])
    assert call() == b"r2"
    assert sleeps == [3.0]
    call, _, sleeps = run([(503, {"Retry-After": "100"}), (200, {})])
    assert call() == b"r2"
    assert sleeps == [8.0]


def test_gives_up_after_the_last_attempt():
    call, calls, sleeps = run([(503, {})], attempts=3)
    with pytest.raises(aiohttp.ClientResponseError) as info:
        call()
    assert info.value.status == 503
    assert len(calls) == 3
    assert sleeps == [0.5, 1.0]


def test_client_errors_are_not_retried():
    call, calls, sleeps = run([(404, {})])
    with pytest.raises(aiohttp.ClientResponseError) as info:
        call()
    assert info.value.status == 404
    assert len(calls) == 1
    assert sleeps == []


def test_connection_errors_are_retried_then_raised():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    sleeps = []

    async def fake_sleep(seconds):
        sleeps.append(seconds)

    async def main():
        async with aiohttp.ClientSession() as session:
            await get_with_retry(
                session, f"http://127.0.0.1:{port}/", attempts=2, sleep=fake_sleep
            )

    with pytest.raises(aiohttp.ClientConnectionError):
        asyncio.run(main())
    assert sleeps == [0.5]


def test_attempts_must_be_positive():
    call, calls, _ = run([(200, {})], attempts=0)
    with pytest.raises(ValueError, match="at least 1"):
        call()
    assert calls == []
