from typing import Any, Iterator, Optional
from dataclasses import dataclass


@dataclass(frozen=True)
class StreamSummary:
    text: str
    stop_reason: str
    is_truncated: bool
    chunk_count: int


class AnthropicStreamHandler:
    def __init__(self, client: Any) -> None:
        self.client = client

    def collect_stream(
        self,
        model: str = "claude-3-5-sonnet-20241022",
        max_tokens: int = 1024,
        messages: list[dict[str, str]] | None = None,
    ) -> StreamSummary:
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
            is_truncated = (stop_reason == "max_tokens")

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
        with self.client.messages.stream(
            model=model,
            max_tokens=max_tokens,
            messages=messages,
        ) as stream:
            for chunk in stream.text_stream:
                yield chunk
