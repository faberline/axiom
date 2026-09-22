"""Requests timeout enforcing HTTP adapter and session factory."""
from typing import Any, Optional, Tuple, Union
import requests
from requests.adapters import HTTPAdapter

TimeoutValue = Union[float, int, Tuple[Union[float, int], Union[float, int]]]


class TimeoutHTTPAdapter(HTTPAdapter):
    """HTTPAdapter that enforces default connect and read timeouts if none specified."""

    def __init__(
        self,
        default_timeout: TimeoutValue = (3.05, 27.0),
        *args: Any,
        **kwargs: Any,
    ) -> None:
        self.default_timeout = self._validate_timeout(default_timeout)
        super().__init__(*args, **kwargs)

    @staticmethod
    def _validate_timeout(timeout: Any) -> TimeoutValue:
        if isinstance(timeout, tuple):
            if len(timeout) != 2:
                raise ValueError("Timeout tuple must contain exactly (connect, read) values")
            connect, read = timeout
            if not isinstance(connect, (int, float)) or not isinstance(read, (int, float)):
                raise TypeError("Timeout values must be numeric")
            if connect <= 0 or read <= 0:
                raise ValueError("Timeout values must be strictly positive")
            return (float(connect), float(read))
        elif isinstance(timeout, (int, float)):
            if timeout <= 0:
                raise ValueError("Timeout must be strictly positive")
            return float(timeout)
        raise TypeError("Timeout must be a float or a (connect, read) tuple")

    def send(
        self,
        request: requests.PreparedRequest,
        stream: bool = False,
        timeout: Any = None,
        verify: Any = True,
        cert: Any = None,
        proxies: Any = None,
    ) -> requests.Response:
        """Send request enforcing default timeout when timeout is None."""
        effective_timeout = self.default_timeout
        return super().send(
            request,
            stream=stream,
            timeout=effective_timeout,
            verify=verify,
            cert=cert,
            proxies=proxies,
        )


def create_timeout_session(
    default_timeout: TimeoutValue = (3.05, 27.0),
) -> requests.Session:
    """Create a requests.Session with TimeoutHTTPAdapter mounted on http:// and https://."""
    session = requests.Session()
    adapter = TimeoutHTTPAdapter(default_timeout=default_timeout)
    session.mount("http://", adapter)
    session.mount("https://", adapter)
    return session
