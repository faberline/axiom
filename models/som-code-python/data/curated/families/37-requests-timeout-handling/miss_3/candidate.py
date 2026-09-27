"""Requests timeout enforcing HTTP adapter and session factory."""

from typing import Any

import requests
from requests.adapters import HTTPAdapter

type TimeoutValue = float | tuple[float, float]


class TimeoutHTTPAdapter(HTTPAdapter):
    """HTTPAdapter that enforces default connect and read timeouts if none specified."""

    def __init__(
        self,
        default_timeout: TimeoutValue = (3.05, 27.0),
        **kwargs: Any,
    ) -> None:
        self.default_timeout = self._validate_timeout(default_timeout)
        super().__init__(**kwargs)

    @staticmethod
    def _validate_timeout(timeout: Any) -> TimeoutValue:
        if isinstance(timeout, tuple):
            if len(timeout) != 2:
                raise ValueError(
                    "Timeout tuple must contain exactly (connect, read) values"
                )
            connect, read = timeout
            if not isinstance(connect, (int, float)) or not isinstance(
                read, (int, float)
            ):
                raise TypeError("Timeout values must be numeric")
            if connect < 0 or read < 0:
                raise ValueError("Timeout values must be strictly positive")
            return (float(connect), float(read))
        if isinstance(timeout, (int, float)):
            if timeout < 0:
                raise ValueError("Timeout must be strictly positive")
            return float(timeout)
        raise TypeError("Timeout must be a float or a (connect, read) tuple")

    def send(
        self,
        request: requests.PreparedRequest,
        *args: Any,
        **kwargs: Any,
    ) -> requests.Response:
        """Send request enforcing default timeout when timeout is None."""
        timeout = kwargs.get("timeout")
        if timeout is None:
            kwargs["timeout"] = self.default_timeout
        else:
            kwargs["timeout"] = self._validate_timeout(timeout)
        return super().send(request, *args, **kwargs)


def create_timeout_session(
    default_timeout: TimeoutValue = (3.05, 27.0),
) -> requests.Session:
    """Return a session with a TimeoutHTTPAdapter mounted for http and https."""
    session = requests.Session()
    adapter = TimeoutHTTPAdapter(default_timeout=default_timeout)
    session.mount("http://", adapter)
    session.mount("https://", adapter)
    return session
