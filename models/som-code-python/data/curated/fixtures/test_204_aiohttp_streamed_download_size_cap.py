import asyncio

import aiohttp
import pytest
from aiohttp import web
from aiohttp.test_utils import TestServer

from candidate import TooLargeError, download


async def exact(request):
    return web.Response(body=b"a" * 2000)


async def streamed(request):
    resp = web.StreamResponse()
    await resp.prepare(request)
    for _ in range(3):
        await resp.write(b"b" * 1000)
        await asyncio.sleep(0.01)
    await resp.write_eof()
    return resp


async def announced(request):
    resp = web.StreamResponse()
    resp.content_length = 5000
    await resp.prepare(request)
    await resp.write(b"c" * 500)
    await asyncio.sleep(0.2)
    request.transport.close()
    return resp


async def missing(request):
    return web.Response(status=404, text="nope")


def fetch(path, dest, max_bytes):
    async def main():
        app = web.Application()
        app.router.add_get("/exact", exact)
        app.router.add_get("/streamed", streamed)
        app.router.add_get("/announced", announced)
        app.router.add_get("/missing", missing)
        async with TestServer(app) as server, aiohttp.ClientSession() as session:
            url = str(server.make_url(path))
            return await download(session, url, dest, max_bytes=max_bytes)

    return asyncio.run(main())


def test_body_at_the_cap_is_written(tmp_path):
    dest = tmp_path / "out.bin"
    assert fetch("/exact", dest, 2000) == 2000
    assert dest.read_bytes() == b"a" * 2000


def test_existing_file_is_replaced(tmp_path):
    dest = tmp_path / "out.bin"
    dest.write_bytes(b"old contents")
    fetch("/exact", dest, 4096)
    assert dest.read_bytes() == b"a" * 2000


def test_streamed_body_over_the_cap_is_removed(tmp_path):
    dest = tmp_path / "out.bin"
    with pytest.raises(TooLargeError, match="exceeds 2500 bytes") as info:
        fetch("/streamed", dest, 2500)
    assert info.value.limit == 2500
    assert not dest.exists()


def test_announced_length_is_rejected_before_reading(tmp_path):
    dest = tmp_path / "out.bin"
    with pytest.raises(TooLargeError):
        fetch("/announced", dest, 1000)
    assert not dest.exists()


def test_http_errors_raise_without_writing(tmp_path):
    dest = tmp_path / "out.bin"
    with pytest.raises(aiohttp.ClientResponseError) as info:
        fetch("/missing", dest, 1000)
    assert info.value.status == 404
    assert not dest.exists()
