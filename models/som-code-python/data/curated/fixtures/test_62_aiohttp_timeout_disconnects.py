"""Oracle test suite for 62-aiohttp-timeout-disconnects."""
import asyncio
from unittest.mock import AsyncMock, MagicMock
import pytest
import aiohttp
import candidate


class MockResponseContext:
    def __init__(self, response):
        self.response = response

    async def __aenter__(self):
        return self.response

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        pass


def test_cancellation_propagates_immediately():
    async def _run():
        fetcher = candidate.ResilientAiohttpFetcher(max_retries=3)
        session = MagicMock(spec=aiohttp.ClientSession)

        def raise_cancelled(*args, **kwargs):
            raise asyncio.CancelledError()

        session.get.side_effect = raise_cancelled

        with pytest.raises(asyncio.CancelledError):
            await fetcher.fetch_json(session, "https://api.example.com/item")

    asyncio.run(_run())


def test_timeout_config_passed_to_session_get():
    async def _run():
        fetcher = candidate.ResilientAiohttpFetcher(total_timeout=7.5, connect_timeout=1.5)
        session = MagicMock(spec=aiohttp.ClientSession)

        resp = MagicMock()
        resp.status = 200
        resp.json = AsyncMock(return_value={"status": "ok"})
        session.get.return_value = MockResponseContext(resp)

        res = await fetcher.fetch_json(session, "https://api.example.com/item")
        assert res == {"status": "ok"}
        assert session.get.call_count == 1
        call_kwargs = session.get.call_args[1]
        assert "timeout" in call_kwargs
        timeout_arg = call_kwargs["timeout"]
        assert isinstance(timeout_arg, aiohttp.ClientTimeout)
        assert timeout_arg.total == 7.5
        assert timeout_arg.connect == 1.5

    asyncio.run(_run())


def test_retries_on_client_connector_error():
    async def _run():
        fetcher = candidate.ResilientAiohttpFetcher(max_retries=2)
        session = MagicMock(spec=aiohttp.ClientSession)

        resp = MagicMock()
        resp.status = 200
        resp.json = AsyncMock(return_value={"data": 42})

        session.get.side_effect = [
            aiohttp.ClientConnectorError(connection_key=MagicMock(), os_error=OSError("connection reset")),
            MockResponseContext(resp),
        ]

        result = await fetcher.fetch_json(session, "https://api.example.com/retry")
        assert result == {"data": 42}
        assert session.get.call_count == 2

    asyncio.run(_run())


def test_max_retries_zero_executes_single_attempt():
    async def _run():
        fetcher = candidate.ResilientAiohttpFetcher(max_retries=0)
        session = MagicMock(spec=aiohttp.ClientSession)

        resp = MagicMock()
        resp.status = 200
        resp.json = AsyncMock(return_value={"count": 1})
        session.get.return_value = MockResponseContext(resp)

        res = await fetcher.fetch_json(session, "https://api.example.com/single")
        assert res == {"count": 1}
        assert session.get.call_count == 1

    asyncio.run(_run())


def test_invalid_parameters():
    with pytest.raises(ValueError):
        candidate.ResilientAiohttpFetcher(max_retries=-1)
    with pytest.raises(ValueError):
        candidate.ResilientAiohttpFetcher(total_timeout=0)
