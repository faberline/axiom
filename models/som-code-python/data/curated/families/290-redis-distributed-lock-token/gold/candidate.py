"""A Redis lock that only its holder can release or extend, via a random token."""

from __future__ import annotations

import secrets
from types import TracebackType
from typing import Protocol

RELEASE_SCRIPT = (
    "if redis.call('get', KEYS[1]) == ARGV[1] then "
    "return redis.call('del', KEYS[1]) else return 0 end"
)
EXTEND_SCRIPT = (
    "if redis.call('get', KEYS[1]) == ARGV[1] then "
    "return redis.call('pexpire', KEYS[1], ARGV[2]) else return 0 end"
)


class LockClient(Protocol):
    """The subset of redis.Redis the lock uses."""

    def set(self, name: str, value: str, *, nx: bool, px: int) -> bool | None:
        """Set a key with options."""

    def eval(self, script: str, numkeys: int, *args: str | int) -> int:
        """Run a Lua script."""


class LockError(Exception):
    """Raised when the lock is busy, not held, or was lost."""


class Lock:
    """An expiring mutual-exclusion lock identified by a per-acquire token."""

    def __init__(self, client: LockClient, name: str, ttl_ms: int) -> None:
        if ttl_ms <= 0:
            raise ValueError("ttl_ms must be positive")
        self._client = client
        self._name = f"lock:{name}"
        self._ttl_ms = ttl_ms
        self._token: str | None = None

    def acquire(self) -> bool:
        """Try once to take the lock and report whether it was taken."""
        token = secrets.token_hex(16)
        taken = self._client.set(self._name, token, nx=True, px=self._ttl_ms)
        if taken:
            self._token = token
        return bool(taken)

    def release(self) -> None:
        """Release the lock if this holder still owns it, else raise LockError."""
        if self._token is None:
            raise LockError("lock is not held")
        token, self._token = self._token, None
        if not self._client.eval(RELEASE_SCRIPT, 1, self._name, token):
            raise LockError("lock expired or was taken by another holder")

    def extend(self) -> None:
        """Reset the lock's TTL if this holder still owns it, else raise LockError."""
        if self._token is None:
            raise LockError("lock is not held")
        if not self._client.eval(
            EXTEND_SCRIPT, 1, self._name, self._token, self._ttl_ms
        ):
            self._token = None
            raise LockError("lock expired or was taken by another holder")

    def __enter__(self) -> Lock:
        if not self.acquire():
            raise LockError(f"{self._name} is busy")
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        self.release()
