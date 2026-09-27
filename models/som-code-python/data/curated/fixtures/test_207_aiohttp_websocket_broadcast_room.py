import asyncio

import aiohttp
import pytest
from aiohttp.test_utils import TestServer

from candidate import CLIENTS_KEY, make_app


def run(scenario):
    async def main():
        app = make_app()
        async with TestServer(app) as server, aiohttp.ClientSession() as session:
            url = str(server.make_url("/ws"))
            return await scenario(app, session, url)

    return asyncio.run(main())


async def nothing_arrives(ws):
    with pytest.raises(asyncio.TimeoutError):
        await ws.receive(timeout=0.2)


async def wait_for_clients(app, count):
    for _ in range(50):
        if len(app[CLIENTS_KEY]) == count:
            return
        await asyncio.sleep(0.02)
    raise AssertionError(f"expected {count} clients, have {len(app[CLIENTS_KEY])}")


def test_messages_reach_peers_but_not_the_sender():
    async def scenario(app, session, url):
        async with session.ws_connect(url) as a, session.ws_connect(url) as b:
            await wait_for_clients(app, 2)
            await a.send_str("hi")
            assert await b.receive_json(timeout=1) == {"message": "hi"}
            await nothing_arrives(a)

    run(scenario)


def test_length_limit_boundary():
    async def scenario(app, session, url):
        async with session.ws_connect(url) as a, session.ws_connect(url) as b:
            await wait_for_clients(app, 2)
            await a.send_str("x" * 500)
            assert await b.receive_json(timeout=1) == {"message": "x" * 500}
            await a.send_str("y" * 501)
            assert await a.receive_json(timeout=1) == {"error": "message too long"}
            await nothing_arrives(b)

    run(scenario)


def test_binary_frames_close_the_socket():
    async def scenario(app, session, url):
        async with session.ws_connect(url) as a:
            await a.send_bytes(b"\x00\x01")
            msg = await a.receive(timeout=1)
            assert msg.type in {aiohttp.WSMsgType.CLOSE, aiohttp.WSMsgType.CLOSED}
            assert a.close_code == aiohttp.WSCloseCode.UNSUPPORTED_DATA
            await wait_for_clients(app, 0)

    run(scenario)


def test_disconnected_clients_are_forgotten():
    async def scenario(app, session, url):
        async with session.ws_connect(url) as a:
            b = await session.ws_connect(url)
            await wait_for_clients(app, 2)
            await b.close()
            await wait_for_clients(app, 1)
            await a.send_str("still here")
            await nothing_arrives(a)

    run(scenario)


def test_shutdown_closes_clients_with_going_away():
    async def scenario(app, session, url):
        ws = await session.ws_connect(url)
        await wait_for_clients(app, 1)
        return ws

    async def main():
        app = make_app()
        async with aiohttp.ClientSession() as session:
            async with TestServer(app) as server:
                ws = await scenario(app, session, str(server.make_url("/ws")))
                closing = asyncio.create_task(ws.receive(timeout=5))
            await closing
            assert ws.close_code == aiohttp.WSCloseCode.GOING_AWAY

    asyncio.run(main())
