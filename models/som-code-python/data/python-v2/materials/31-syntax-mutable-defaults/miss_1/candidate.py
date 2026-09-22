from dataclasses import dataclass, field
from typing import Any


@dataclass
class AuditEvent:
    event_type: str
    payload: dict[str, Any] = field(default_factory=dict)
    tags: list[str] = field(default_factory=list)
    metadata: dict[str, Any] = field(default_factory=dict)


class AuditLogger:
    def __init__(self) -> None:
        self.events: list[AuditEvent] = []
        self._index: dict[str, list[AuditEvent]] = {}

    def record_event(
        self,
        event_type: str,
        payload: dict[str, Any] | None = None,
        tags: list[str] = [],
        metadata: dict[str, Any] | None = None,
    ) -> AuditEvent:
        if not event_type or not isinstance(event_type, str):
            raise ValueError("event_type must be a non-empty string")
        if tags is not None and not isinstance(tags, list):
            raise TypeError("tags must be a list")
        if metadata is not None and not isinstance(metadata, dict):
            raise TypeError("metadata must be a dict")
        if payload is not None and not isinstance(payload, dict):
            raise TypeError("payload must be a dict")

        safe_tags = tags
        safe_payload = dict(payload) if payload is not None else {}
        safe_metadata = dict(metadata) if metadata is not None else {}

        if len(safe_tags) > 10:
            raise ValueError("tags cannot exceed 10 items")

        event = AuditEvent(
            event_type=event_type,
            payload=safe_payload,
            tags=safe_tags,
            metadata=safe_metadata,
        )
        self.events.append(event)
        for tag in safe_tags:
            self._index.setdefault(tag, []).append(event)
        return event

    def get_events_by_tag(self, tag: str) -> list[AuditEvent]:
        return list(self._index.get(tag, []))

    def clear(self) -> None:
        self.events.clear()
        self._index.clear()

    def count(self) -> int:
        return len(self.events)
