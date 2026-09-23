"""Oracle test suite for 61-aiohttp-session-lifecycle."""
import asyncio
from unittest.mock import AsyncMock
import pytest
import candidate


def test_session_lifecycle_context_manager():
    async def _run():
        client = candidate.ManagedAiohttpClient(base_url="https://api.example.com", timeout_seconds=10.0)
        session = None
        async with client as c:
            session = await c.get_session()
            assert session is not None
            assert not c.is_closed
            orig_close = session.close
            close_mock = AsyncMock(side_effect=orig_close)
            session.close = close_mock

        assert client.is_closed
        assert close_mock.await_count == 1, "session.close() must be awaited upon exiting context"

    asyncio.run(_run())


def test_explicit_async_close():
    async def _run():
        client = candidate.ManagedAiohttpClient()
        session = await client.get_session()
        orig_close = session.close
        close_mock = AsyncMock(side_effect=orig_close)
        session.close = close_mock

        await client.close()
        assert close_mock.await_count == 1, "client.close() must await session.close()"
        assert client.is_closed

    asyncio.run(_run())


def test_reopen_closed_client_raises():
    async def _run():
        client = candidate.ManagedAiohttpClient()
        await client.get_session()
        await client.close()
        with pytest.raises(RuntimeError, match="closed"):
            await client.get_session()

    asyncio.run(_run())


def test_session_instance_reuse():
    async def _run():
        client = candidate.ManagedAiohttpClient()
        s1 = await client.get_session()
        s2 = await client.get_session()
        assert s1 is s2, "get_session() must reuse active session instead of allocating new ones"
        await client.close()

    asyncio.run(_run())


def test_timeout_validation():
    with pytest.raises(ValueError, match="timeout_seconds"):
        candidate.ManagedAiohttpClient(timeout_seconds=0)
    with pytest.raises(ValueError, match="timeout_seconds"):
        candidate.ManagedAiohttpClient(timeout_seconds=-5.0)
