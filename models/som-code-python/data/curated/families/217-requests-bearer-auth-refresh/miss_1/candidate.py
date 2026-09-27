"""Bearer token auth for requests that refreshes the token once on 401."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

import requests
from requests.auth import AuthBase


class TokenAuth(AuthBase):
    """Attach a cached bearer token and retry once with a fresh one on 401."""

    def __init__(self, fetch_token: Callable[[], str]) -> None:
        self._fetch_token = fetch_token
        self._token: str | None = None

    def _current(self) -> str:
        if self._token is None:
            self._token = self._fetch_token()
        return self._token

    def __call__(self, r: requests.PreparedRequest) -> requests.PreparedRequest:
        r.headers["Authorization"] = f"Bearer {self._current()}"
        r.hooks["response"].append(self._retry_on_401)
        return r

    def _retry_on_401(
        self, resp: requests.Response, **kwargs: Any
    ) -> requests.Response:
        if resp.status_code != 401:
            return resp
        retry = resp.request.copy()
        retry.headers["Authorization"] = f"Bearer {self._current()}"
        resp.close()
        new: requests.Response = resp.connection.send(retry, **kwargs)
        new.history.append(resp)
        new.request = retry
        return new
