"""Aiohttp managed client session lifecycle."""

from types import TracebackType
from typing import Self

import aiohttp


class ManagedAiohttpClient:
    """Own one aiohttp.ClientSession, opened lazily and closed exactly once."""

    def __init__(self, base_url: str = "", timeout_seconds: float = 30.0) -> None:
        self.base_url = base_url.rstrip("/")
        self.timeout = aiohttp.ClientTimeout(total=timeout_seconds)
        self._session: aiohttp.ClientSession | None = None
        self._closed: bool = False

    @property
    def is_closed(self) -> bool:
        """Whether the client was closed or its session has closed underneath it."""
        return self._closed or (self._session is not None and self._session.closed)

    async def get_session(self) -> aiohttp.ClientSession:
        """Return the active session or lazily create a new one if not closed."""
        if self._closed:
            raise RuntimeError("Cannot get session on closed ManagedAiohttpClient")
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession(
                base_url=self.base_url or None,
                timeout=self.timeout,
            )
        return self._session

    async def close(self) -> None:
        """Asynchronously close the active session and mark client as closed."""
        if self._session is not None and not self._session.closed:
            await self._session.close()
            self._session = None
        self._closed = True

    async def __aenter__(self) -> Self:
        """Open the session and return the client."""
        await self.get_session()
        return self

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        """Close the session whether or not the block raised."""
        await self.close()
