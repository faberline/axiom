"""Wrap an async OpenAI-style client for single and streamed chat completions."""

from collections.abc import AsyncIterator
from typing import Any


class AsyncOpenAIChatService:
    """Chat completions over an injected async client with a fixed timeout."""

    def __init__(self, client: Any = None, timeout: float = 30.0) -> None:
        if timeout <= 0:
            raise ValueError("timeout must be positive")
        self.client = client
        self.timeout = timeout

    async def generate_chat_response(
        self,
        messages: list[dict[str, str]],
        model: str = "gpt-4o-mini",
        temperature: float = 0.7,
    ) -> str:
        """Return the first choice's content, or an empty string when absent."""
        if not messages:
            raise ValueError("messages cannot be empty")
        response = await self.client.completions.create(
            model=model,
            messages=messages,
            temperature=temperature,
            timeout=self.timeout,
        )
        choice = response.choices[0]
        return choice.message.content or ""

    async def stream_chat_response(
        self,
        messages: list[dict[str, str]],
        model: str = "gpt-4o-mini",
    ) -> AsyncIterator[str]:
        """Yield each non-empty content delta from a streamed completion."""
        if not messages:
            raise ValueError("messages cannot be empty")
        stream = await self.client.chat.completions.create(
            model=model,
            messages=messages,
            stream=True,
            timeout=self.timeout,
        )
        async for chunk in stream:
            if chunk.choices and chunk.choices[0].delta.content:
                yield chunk.choices[0].delta.content
