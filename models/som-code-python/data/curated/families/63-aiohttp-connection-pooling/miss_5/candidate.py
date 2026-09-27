"""Pooled aiohttp session and concurrency manager."""

import asyncio

import aiohttp


class PooledAiohttpManager:
    """Manages connection pooling and concurrency limiting for aiohttp requests."""

    def __init__(
        self,
        max_connections: int = 10,
        limit_per_host: int = 5,
        timeout_seconds: float = 15.0,
    ) -> None:
        if max_connections <= 0:
            raise ValueError("max_connections must be positive")
        if limit_per_host <= 0:
            raise ValueError("limit_per_host must be positive")
        if limit_per_host > max_connections:
            raise ValueError("limit_per_host cannot exceed max_connections")

        self.max_connections = max_connections
        self.limit_per_host = limit_per_host
        self.timeout = aiohttp.ClientTimeout(total=timeout_seconds)
        self.connector = aiohttp.TCPConnector(
            limit=max_connections,
            limit_per_host=limit_per_host,
            enable_cleanup_closed=True,
        )
        self.semaphore = asyncio.Semaphore(max_connections)
        self._session: aiohttp.ClientSession | None = None

    async def get_session(self) -> aiohttp.ClientSession:
        """Return or create a shared ClientSession using the bounded TCPConnector."""
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession(
                connector=self.connector,
                connector_owner=False,
                timeout=self.timeout,
            )
        return self._session

    async def fetch_one(self, url: str) -> str:
        """Fetch a single URL bounded by the concurrency semaphore and shared pool."""
        session = await self.get_session()
        async with session.get(url) as response:
            return await response.text()

    async def fetch_batch(self, urls: list[str]) -> list[str]:
        """Fetch multiple URLs concurrently reusing the connection pool."""
        tasks = [self.fetch_one(url) for url in urls]
        return await asyncio.gather(*tasks)

    async def close(self) -> None:
        """Gracefully close the session and underlying TCP connector."""
        if self._session is not None and not self._session.closed:
            await self._session.close()
            self._session = None
        if not self.connector.closed:
            await self.connector.close()
