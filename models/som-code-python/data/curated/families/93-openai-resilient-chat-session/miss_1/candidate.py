"""Keep a chat session alive through context overflows and rate limits."""

import time
from typing import Any


class ResilientChatSession:
    """Chat history that prunes on context overflow and backs off on rate limits."""

    def __init__(
        self,
        client: Any,
        model: str = "gpt-4o",
        max_retries: int = 3,
        base_backoff: float = 0.05,
        max_backoff: float = 1.0,
    ) -> None:
        self.client = client
        self.model = model
        self.max_retries = max_retries
        self.base_backoff = base_backoff
        self.max_backoff = max_backoff
        self.messages: list[dict[str, str]] = []

    def add_message(self, role: str, content: str) -> None:
        """Append a message with role and content to the history."""
        self.messages.append({"role": role, "content": content})

    def prune_history_for_context(self) -> bool:
        """Drop the oldest exchange, keeping any system prompt; False when too short."""
        if len(self.messages) <= 2:
            return False
        has_system = bool(self.messages and self.messages[0].get("role") == "system")
        if not has_system:
            if len(self.messages) <= 3:
                return False
            self.messages = [self.messages[0], *self.messages[3:]]
            return True
        self.messages = self.messages[2:]
        return True

    def send_message(self, prompt: str) -> str:
        """Send prompt, retrying on overflow and rate limits, and return the reply."""
        self.add_message("user", prompt)
        attempt = 0
        while True:
            try:
                response = self.client.chat.completions.create(
                    model=self.model,
                    messages=self.messages,
                )
                content = response.choices[0].message.content or ""
                self.add_message("assistant", content)
                return content
            except Exception as exc:
                err_type = exc.__class__.__name__
                err_msg = str(exc).lower()

                if (
                    "context_length_exceeded" in err_msg
                    or "contextwindow" in err_type.lower()
                ):
                    if self.prune_history_for_context():
                        continue
                    raise RuntimeError(
                        "Context window exceeded and cannot prune further"
                    ) from exc

                if "ratelimit" in err_type.lower() or "429" in err_msg:
                    attempt += 1
                    if attempt > self.max_retries:
                        raise RuntimeError(
                            f"Max retries ({self.max_retries}) exceeded on rate limit"
                        ) from exc
                    delay = min(
                        self.max_backoff, self.base_backoff * (2 ** (attempt - 1))
                    )
                    time.sleep(delay)
                    continue

                raise
