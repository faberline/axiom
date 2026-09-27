"""Build OpenAI Batch API input files and read their output files."""

from __future__ import annotations

import json
from collections.abc import Iterable

ENDPOINT = "/v1/chat/completions"
Messages = list[dict[str, str]]


class DuplicateCustomIdError(ValueError):
    """Raised when two batch requests share one custom_id."""


def build_batch(items: Iterable[tuple[str, Messages]], *, model: str) -> str:
    """Return JSONL with one chat completion request per (custom_id, messages)."""
    seen: set[str] = set()
    lines: list[str] = []
    for custom_id, messages in items:
        if custom_id in seen:
            raise DuplicateCustomIdError(f"duplicate custom_id {custom_id!r}")
        seen.add(custom_id)
        request = {
            "custom_id": custom_id,
            "method": "POST",
            "url": ENDPOINT,
            "body": {"model": model, "messages": messages},
        }
        lines.append(json.dumps(request))
    return "\n".join(lines)


def read_results(text: str) -> tuple[dict[str, str], dict[str, str]]:
    """Return (answer content by custom_id, error message by custom_id)."""
    answers: dict[str, str] = {}
    errors: dict[str, str] = {}
    for line in text.splitlines():
        if not line.strip():
            continue
        record = json.loads(line)
        custom_id = record["custom_id"]
        response = record.get("response")
        if record.get("error"):
            errors[custom_id] = record["error"]["message"]
        elif response["status_code"] != 200:
            errors[custom_id] = response["body"]["error"]["message"]
        else:
            body = response["body"]
            answers[custom_id] = body["choices"][0]["message"]["content"]
    return answers, errors
