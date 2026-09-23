"""Requests retry adapter configuration and session factory."""

from collections.abc import Collection

import requests
from requests.adapters import HTTPAdapter
from urllib3.util import Retry

DEFAULT_STATUS_FORCELIST = (429, 500, 502, 503, 504)
SAFE_METHODS = frozenset({"HEAD", "GET", "PUT", "DELETE", "OPTIONS", "TRACE"})


def build_retry_strategy(
    total: int = 3,
    backoff_factor: float = 0.5,
    status_forcelist: Collection[int] = DEFAULT_STATUS_FORCELIST,
    allowed_methods: Collection[str] = SAFE_METHODS,
    raise_on_status: bool = False,
) -> Retry:
    """Build a urllib3 Retry configuration with validation."""
    if total <= 0:
        raise ValueError("total retries must be positive")
    if backoff_factor < 0.0:
        raise ValueError("backoff_factor must be non-negative")

    return Retry(
        total=total,
        backoff_factor=backoff_factor,
        status_forcelist=tuple(status_forcelist),
        allowed_methods=frozenset(allowed_methods),
        raise_on_status=raise_on_status,
    )


def create_retry_session(
    total: int = 3,
    backoff_factor: float = 0.5,
    status_forcelist: Collection[int] = DEFAULT_STATUS_FORCELIST,
    allowed_methods: Collection[str] = SAFE_METHODS,
) -> requests.Session:
    """Return a session that retries through an HTTPAdapter on http and https."""
    retry_strategy = build_retry_strategy(
        total=total,
        backoff_factor=backoff_factor,
        status_forcelist=status_forcelist,
        allowed_methods=allowed_methods,
    )
    adapter = HTTPAdapter(max_retries=retry_strategy)
    session = requests.Session()
    session.mount("http://", adapter)
    session.mount("https://", adapter)
    return session
