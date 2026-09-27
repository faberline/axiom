"""Oracle test suite for 64-aiohttp-response-release."""
import asyncio
from unittest.mock import AsyncMock, MagicMock
import pytest
import aiohttp
import candidate


def test_release_called_on_bad_status():
    async def _run():
        consumer = candidate.ResponseStreamConsumer()
        session = MagicMock(spec=aiohttp.ClientSession)

        resp = MagicMock()
        resp.status = 500
        resp.release = AsyncMock()
        session.get = AsyncMock(return_value=resp)

        with pytest.raises(candidate.BadStatusError):
            await consumer.fetch_json_safe(session, "https://api.example.com/fail")

        assert resp.release.await_count >= 1, "response.release() must be awaited on bad HTTP status"

    asyncio.run(_run())


def test_release_called_on_json_parse_error():
    async def _run():
        consumer = candidate.ResponseStreamConsumer()
        session = MagicMock(spec=aiohttp.ClientSession)

        resp = MagicMock()
        resp.status = 200
        resp.json = AsyncMock(side_effect=ValueError("Invalid JSON"))
        resp.release = AsyncMock()
        session.get = AsyncMock(return_value=resp)

        with pytest.raises(ValueError, match="Invalid JSON"):
            await consumer.fetch_json_safe(session, "https://api.example.com/bad_json")

        assert resp.release.await_count >= 1, "response.release() must be awaited when JSON parsing fails"

    asyncio.run(_run())


def test_header_reading_releases_connection():
    async def _run():
        consumer = candidate.ResponseStreamConsumer()
        session = MagicMock(spec=aiohttp.ClientSession)

        resp = MagicMock()
        resp.headers = {"X-Custom-Token": "secret-123"}
        resp.release = AsyncMock()
        session.get = AsyncMock(return_value=resp)

        val = await consumer.read_header_value(session, "https://api.example.com/head", "X-Custom-Token")
        assert val == "secret-123"
        assert resp.release.await_count == 1, "response.release() must be awaited after reading header"

    asyncio.run(_run())


def test_successful_fetch_releases_or_reads_cleanly():
    async def _run():
        consumer = candidate.ResponseStreamConsumer()
        session = MagicMock(spec=aiohttp.ClientSession)

        resp = MagicMock()
        resp.status = 200
        resp.json = AsyncMock(return_value={"ok": True})
        resp.release = AsyncMock()
        resp.close = MagicMock()
        session.get = AsyncMock(return_value=resp)

        data = await consumer.fetch_json_safe(session, "https://api.example.com/ok")
        assert data == {"ok": True}
        assert resp.close.call_count == 0, "response.close() should not be used in place of release"

    asyncio.run(_run())
