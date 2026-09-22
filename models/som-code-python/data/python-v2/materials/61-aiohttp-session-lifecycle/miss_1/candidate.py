"""Aiohttp managed client session lifecycle."""
from typing import Any, Optional
import aiohttp


class ManagedAiohttpClient:
    """Manages an aiohttp.ClientSession with async context management and lifecycle tracking."""

    def __init__(self, base_url: str = "", timeout_seconds: float = 30.0) -> None:
        if timeout_seconds <= 0:
            raise ValueError("timeout_seconds must be positive")
        self.base_url = base_url.rstrip("/")
        self.timeout = aiohttp.ClientTimeout(total=timeout_seconds)
        self._session: Optional[aiohttp.ClientSession] = None
        self._closed: bool = False

    @property
    def is_closed(self) -> bool:
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
            self._session = None
        self._closed = True

    async def __aenter__(self) -> "ManagedAiohttpClient":
        await self.get_session()
        return self

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        await self.close()
