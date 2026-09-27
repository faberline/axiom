"""Embed many texts through an OpenAI client in ordered, bounded batches."""

from __future__ import annotations

from typing import Any


class EmbeddingCountError(RuntimeError):
    """Raised when a batch returns a different number of embeddings."""


def embed_all(
    client: Any, texts: list[str], *, model: str, batch_size: int = 100
) -> list[list[float]]:
    """Return one embedding per text, in the order of ``texts``."""
    if batch_size < 1:
        raise ValueError("batch_size must be positive")
    if any(not text.strip() for text in texts):
        raise ValueError("texts must be non-empty")
    vectors: list[list[float]] = []
    for start in range(0, len(texts), batch_size):
        batch = texts[start : start + batch_size]
        response = client.embeddings.create(model=model, input=batch)
        data = sorted(response["data"], key=lambda item: item["index"])
        if len(data) != len(batch):
            raise EmbeddingCountError(
                f"expected {len(batch)} embeddings, got {len(data)}"
            )
        vectors.extend(item["embedding"] for item in data)
    return vectors
