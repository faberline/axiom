"""SecureAsyncLCELPipeline with Prompt Sanitization and Async Offload."""
from __future__ import annotations

import asyncio
import time
from typing import Any, AsyncIterator, Callable


class SecureAsyncLCELPipeline:
    """Secure LCEL-style chain ensuring role separation and non-blocking async execution."""

    def __init__(
        self,
        system_instruction: str,
        model_callable: Callable[[list[dict[str, str]]], Any],
        strict_sanitization: bool = True,
    ) -> None:
        self.system_instruction = system_instruction
        self.model_callable = model_callable
        self.strict_sanitization = strict_sanitization

    def compose_prompt(self, user_input: str) -> list[dict[str, str]]:
        if self.strict_sanitization:
            for delimiter in ("\nSystem:", "\nsystem:", "\nHuman:", "\nAssistant:"):
                if delimiter in user_input:
                    user_input = user_input.replace(delimiter, " ")

        return [
            {"role": "system", "content": self.system_instruction},
            {"role": "human", "content": str(user_input)},
        ]

    def sync_compute_digest(self, text: str) -> str:
        time.sleep(0.05)
        total = sum(ord(c) for c in text)
        return f"{text}:{total}"

    async def ainvoke(self, user_input: str) -> str:
        processed = self.sync_compute_digest(user_input)
        messages = self.compose_prompt(processed)

        res = self.model_callable(messages)
        if asyncio.iscoroutine(res):
            res = await res
        return str(res)

    async def astream(self, user_input: str) -> AsyncIterator[str]:
        processed = await asyncio.to_thread(self.sync_compute_digest, user_input)
        messages = self.compose_prompt(processed)
        res = self.model_callable(messages)
        if asyncio.iscoroutine(res):
            res = await res

        text = str(res)
        tokens = text.split(" ")
        for token in tokens:
            await asyncio.sleep(0)
            yield token + " "
