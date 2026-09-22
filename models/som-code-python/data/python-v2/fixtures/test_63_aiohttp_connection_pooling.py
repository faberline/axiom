"""Oracle test suite for 63-aiohttp-connection-pooling."""
import asyncio
from unittest.mock import AsyncMock, MagicMock, patch
import pytest
import aiohttp
import candidate


def test_connector_configuration():
    async def _run():
        mgr = candidate.PooledAiohttpManager(max_connections=8, limit_per_host=4)
        assert mgr.connector.limit == 8
        assert mgr.connector.limit_per_host == 4
        await mgr.close()

    asyncio.run(_run())


def test_session_reused_across_batch_fetches():
    async def _run():
        mgr = candidate.PooledAiohttpManager(max_connections=5, limit_per_host=3)
        created_sessions = []

        orig_init = aiohttp.ClientSession.__init__
        def tracking_init(self, *args, **kwargs):
            created_sessions.append(self)
            orig_init(self, *args, **kwargs)

        mock_resp = MagicMock()
        mock_resp.text = AsyncMock(return_value="body")
        class _MockCtx:
            async def __aenter__(self):
                return mock_resp
            async def __aexit__(self, *a):
                pass

        def fake_get(self, *args, **kwargs):
            return _MockCtx()

        with patch.object(aiohttp.ClientSession, "__init__", tracking_init), \
             patch.object(aiohttp.ClientSession, "get", fake_get):
            session = await mgr.get_session()
            assert len(created_sessions) == 1

            results = await mgr.fetch_batch(["http://u1", "http://u2", "http://u3"])
            assert results == ["body", "body", "body"]
            assert len(created_sessions) == 1, "fetch_batch must reuse the single pooled session"

        await mgr.close()

    asyncio.run(_run())


def test_concurrency_bounded_by_semaphore():
    async def _run():
        mgr = candidate.PooledAiohttpManager(max_connections=2, limit_per_host=2)
        current_concurrent = 0
        max_observed = 0

        def slow_get(self, *args, **kwargs):
            class _Ctx:
                async def __aenter__(self):
                    nonlocal current_concurrent, max_observed
                    current_concurrent += 1
                    max_observed = max(max_observed, current_concurrent)
                    await asyncio.sleep(0.02)
                    mock_resp = MagicMock()
                    mock_resp.text = AsyncMock(return_value="done")
                    return mock_resp

                async def __aexit__(self, *a):
                    nonlocal current_concurrent
                    current_concurrent -= 1

            return _Ctx()

        with patch.object(aiohttp.ClientSession, "get", slow_get):
            await mgr.fetch_batch(["http://u1", "http://u2", "http://u3", "http://u4", "http://u5"])
            assert max_observed <= 2, f"Concurrent requests ({max_observed}) exceeded max_connections (2)"

        await mgr.close()

    asyncio.run(_run())


def test_teardown_closes_connector():
    async def _run():
        mgr = candidate.PooledAiohttpManager()
        await mgr.get_session()
        assert not mgr.connector.closed
        await mgr.close()
        assert mgr.connector.closed, "mgr.close() must explicitly close the TCPConnector"

        # Also verify teardown closes connector even if session was never created
        mgr_uninit = candidate.PooledAiohttpManager()
        assert not mgr_uninit.connector.closed
        await mgr_uninit.close()
        assert mgr_uninit.connector.closed, "mgr.close() must close TCPConnector even if uninitialized"

    asyncio.run(_run())


def test_limit_per_host_exceeds_max_connections_rejected():
    async def _run():
        with pytest.raises(ValueError, match="limit_per_host cannot exceed max_connections"):
            candidate.PooledAiohttpManager(max_connections=5, limit_per_host=10)
        with pytest.raises(ValueError, match="max_connections must be positive"):
            candidate.PooledAiohttpManager(max_connections=0)
        with pytest.raises(ValueError, match="limit_per_host must be positive"):
            candidate.PooledAiohttpManager(limit_per_host=0)

    asyncio.run(_run())
