"""Consume Anthropic message streams inside their context manager."""

from collections.abc import Iterator
from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class StreamSummary:
    """Collected text and stop metadata of one finished stream."""

    text: str
    stop_reason: str
    is_truncated: bool
    chunk_count: int


class AnthropicStreamHandler:
    """Stream Anthropic messages while keeping each stream's lifetime scoped."""

    def __init__(self, client: Any) -> None:
        self.client = client

    def collect_stream(
        self,
        model: str = "claude-3-5-sonnet-20241022",
        max_tokens: int = 1024,
        messages: list[dict[str, str]] | None = None,
    ) -> StreamSummary:
        """Drain one stream and summarise its text, stop reason, and truncation."""
        if not messages:
            raise ValueError("messages must not be empty")

        accumulated: list[str] = []
        with self.client.messages.stream(
            model=model,
            max_tokens=max_tokens,
            messages=messages,
        ) as stream:
            for text_chunk in stream.text_stream:
                accumulated.append(text_chunk)

            final_message = stream.get_final_message()
            stop_reason = getattr(final_message, "stop_reason", None) or "end_turn"
            is_truncated = stop_reason == "end_turn"

            return StreamSummary(
                text="".join(accumulated),
                stop_reason=stop_reason,
                is_truncated=is_truncated,
                chunk_count=len(accumulated),
            )

    def stream_text_chunks(
        self,
        model: str = "claude-3-5-sonnet-20241022",
        max_tokens: int = 1024,
        messages: list[dict[str, str]] | None = None,
    ) -> Iterator[str]:
        """Yield text chunks while the underlying stream stays open."""
        if not messages:
            raise ValueError("messages must not be empty")

        with self.client.messages.stream(
            model=model,
            max_tokens=max_tokens,
            messages=messages,
        ) as stream:
            yield from stream.text_stream
