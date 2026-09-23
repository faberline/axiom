"""Requests connection pooling manager."""

from types import TracebackType

import requests
from requests.adapters import HTTPAdapter


class PooledSessionManager:
    """Manages requests.Session with configured HTTP connection pooling."""

    def __init__(
        self,
        pool_connections: int = 10,
        pool_maxsize: int = 20,
        pool_block: bool = True,
    ) -> None:
        if pool_connections <= 0:
            raise ValueError("pool_connections must be positive")
        if pool_maxsize <= 0:
            raise ValueError("pool_maxsize must be positive")
        if pool_maxsize < pool_connections:
            raise ValueError(
                "pool_maxsize must be greater than or equal to pool_connections"
            )

        self.pool_connections = pool_connections
        self.pool_maxsize = pool_maxsize
        self.pool_block = pool_block
        self._session: requests.Session | None = None

    def get_session(self) -> requests.Session:
        """Return the shared session, mounting pooled adapters on first use."""
        if self._session is None:
            session = requests.Session()
            adapter = HTTPAdapter(
                pool_connections=self.pool_connections,
                pool_maxsize=self.pool_maxsize,
                pool_block=self.pool_block,
            )
            session.mount("http://", adapter)
            session.mount("https://", adapter)
            self._session = session
        return self._session

    def close(self) -> None:
        """Close the active session and release pool resources."""
        if self._session is not None:
            self._session = None

    def __enter__(self) -> requests.Session:
        return self.get_session()

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        self.close()
