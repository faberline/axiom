"""Check pass-through identity with sentinel and ignore volatile arguments with ANY."""

from collections.abc import Callable, Iterable
from typing import Protocol
from unittest.mock import call


class Broker(Protocol):
    """A message broker client."""

    def publish(self, topic: str, payload: object, *, headers: dict[str, str]) -> None:
        """Send payload to topic with the given headers."""


def fan_out(
    broker: Broker, topic: str, payloads: Iterable[object], new_id: Callable[[], str]
) -> list[str]:
    """Publish each payload unchanged under a fresh message id and return the ids."""
    if not topic or topic != topic.strip():
        raise ValueError("topic must be a non-empty name without padding")
    ids: list[str] = []
    for payload in payloads:
        message_id = new_id()
        broker.publish(
            topic, payload, headers={"message-id": message_id, "topic": topic}
        )
        ids.append(message_id)
    return ids


def expected_publish(topic: str, payload: object) -> object:
    """The call fan_out makes for payload, matching any generated headers."""
    return call.publish(topic, payload, headers={})
