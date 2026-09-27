"""Derive deterministic idempotency keys with uuid5 and replay repeated requests."""

import json
import uuid
from collections.abc import Callable, Mapping

NAMESPACE = uuid.UUID("6f1c2d0e-8a4b-4c3d-9e5f-1a2b3c4d5e6f")


def idempotency_key(tenant: str, payload: Mapping[str, object]) -> uuid.UUID:
    """Return the same UUID for the same tenant and payload, whatever the key order."""
    name = tenant.strip()
    if not name:
        raise ValueError("tenant must not be blank")
    canonical = json.dumps(payload, sort_keys=True, separators=(",", ":"), default=str)
    return uuid.uuid5(NAMESPACE, f"{name}:{canonical}")


class IdempotencyStore:
    """Remembers the result of each successful request by its idempotency key."""

    def __init__(self) -> None:
        self._results: dict[uuid.UUID, str] = {}

    def run(
        self, tenant: str, payload: Mapping[str, object], action: Callable[[], str]
    ) -> tuple[str, bool]:
        """Run action once per key and return its result with a replayed flag."""
        key = idempotency_key(tenant, payload)
        if key in self._results:
            return self._results[key], True
        self._results[key] = ""
        result = action()
        self._results[key] = result
        return result, False
