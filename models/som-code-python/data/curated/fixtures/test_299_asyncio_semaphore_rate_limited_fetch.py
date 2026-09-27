import asyncio

import pytest

from candidate import Outcome, fetch_all


class Server:
    def __init__(self, delay=0.01, slow=(), broken=()):
        self.delay = delay
        self.slow = set(slow)
        self.broken = set(broken)
        self.active = 0
        self.peak = 0
        self.calls = []

    async def __call__(self, url):
        self.calls.append(url)
        self.active += 1
        self.peak = max(self.peak, self.active)
        try:
            await asyncio.sleep(1.0 if url in self.slow else self.delay)
            if url in self.broken:
                raise ConnectionResetError("reset by peer")
            return url.encode()
        finally:
            self.active -= 1


def run(coro):
    return asyncio.run(coro)


def test_concurrency_never_exceeds_limit():
    server = Server()
    urls = [f"u{i}" for i in range(12)]
    out = run(fetch_all(server, urls, limit=3))
    assert server.peak == 3
    assert [o.url for o in out] == urls
    assert out[5] == Outcome("u5", body=b"u5")


def test_errors_and_timeouts_are_captured_per_url():
    server = Server(slow={"s"}, broken={"b"})
    out = run(fetch_all(server, ["a", "s", "b"], limit=5, timeout=0.1))
    assert out[0] == Outcome("a", body=b"a")
    assert out[1] == Outcome("s", error="timeout")
    assert out[2] == Outcome("b", error="ConnectionResetError: reset by peer")


def test_duplicate_urls_are_fetched_once():
    server = Server()
    out = run(fetch_all(server, ["x", "y", "x"], limit=2))
    assert sorted(server.calls) == ["x", "y"]
    assert [o.url for o in out] == ["x", "y"]


def test_timeout_does_not_hold_a_slot():
    server = Server(slow={"s1", "s2"})
    out = run(fetch_all(server, ["s1", "s2", "a"], limit=2, timeout=0.05))
    assert out[2].body == b"a"


def test_invalid_arguments():
    with pytest.raises(ValueError):
        run(fetch_all(Server(), ["a"], limit=0))
    with pytest.raises(ValueError):
        run(fetch_all(Server(), ["a"], timeout=0))
