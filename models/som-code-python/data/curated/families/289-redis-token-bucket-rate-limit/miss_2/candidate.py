"""A token-bucket rate limiter whose state lives in a Redis hash."""

from __future__ import annotations

import math
import time
from collections.abc import Callable, Mapping, Sequence
from typing import Protocol


class HashClient(Protocol):
    """The subset of redis.Redis the limiter uses."""

    def hmget(self, name: str, keys: Sequence[str]) -> list[bytes | None]:
        """Return the values of several hash fields."""

    def hset(self, name: str, mapping: Mapping[str, float]) -> int:
        """Set several hash fields."""

    def expire(self, name: str, seconds: int) -> bool:
        """Set a key's time to live."""


class TokenBucket:
    """Allow bursts up to capacity and refill at rate tokens per second."""

    def __init__(
        self,
        client: HashClient,
        capacity: int,
        rate: float,
        clock: Callable[[], float] = time.time,
    ) -> None:
        if capacity < 1 or rate <= 0:
            raise ValueError("capacity must be >= 1 and rate > 0")
        self._client = client
        self._capacity = capacity
        self._rate = rate
        self._clock = clock

    def _tokens(self, name: str, now: float) -> float:
        tokens_raw, stamp_raw = self._client.hmget(name, ["tokens", "ts"])
        if tokens_raw is None or stamp_raw is None:
            return float(self._capacity)
        elapsed = now - float(stamp_raw)
        return min(float(self._capacity), float(tokens_raw) + elapsed * self._rate)

    def allow(self, key: str, cost: int = 1) -> bool:
        """Spend cost tokens for key if available and report whether it was allowed."""
        if cost < 1 or cost > self._capacity:
            raise ValueError("cost must be between 1 and capacity")
        name = f"bucket:{key}"
        now = self._clock()
        tokens = self._tokens(name, now)
        allowed = tokens >= cost
        if allowed:
            tokens -= cost
        self._client.hset(name, mapping={"tokens": tokens, "ts": now})
        self._client.expire(name, math.ceil(self._capacity / self._rate))
        return allowed

    def retry_after(self, key: str, cost: int = 1) -> float:
        """Return the seconds until cost tokens will be available for key."""
        tokens = self._tokens(f"bucket:{key}", self._clock())
        return max(0.0, (cost - tokens) / self._rate)
