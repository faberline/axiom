"""A LangChain-style retriever with top-k, a score threshold and dedup."""

from __future__ import annotations

import math
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any


@dataclass(frozen=True)
class Document:
    """A retrieved text with metadata carrying its source id."""

    page_content: str
    metadata: dict[str, Any] = field(default_factory=dict)


def cosine(left: Sequence[float], right: Sequence[float]) -> float:
    """Return cosine similarity, or 0.0 when either vector is zero."""
    norm = math.sqrt(sum(x * x for x in left)) * math.sqrt(sum(y * y for y in right))
    if norm == 0:
        return 0.0
    return sum(x * y for x, y in zip(left, right, strict=True)) / norm


class InMemoryVectorStore:
    """Keep documents with their embeddings and rank them by cosine."""

    def __init__(self, embed: Callable[[str], list[float]]) -> None:
        self._embed = embed
        self._rows: list[tuple[Document, list[float]]] = []

    def add_documents(self, docs: Sequence[Document]) -> None:
        """Embed and store each document in insertion order."""
        for doc in docs:
            self._rows.append((doc, self._embed(doc.page_content)))

    def similarity_search_with_score(self, query: str) -> list[tuple[Document, float]]:
        """Return every document with its score, best first, ties stable."""
        vector = self._embed(query)
        scored = [(doc, cosine(vector, emb)) for doc, emb in self._rows]
        return sorted(scored, key=lambda pair: pair[1], reverse=True)


class ScoreThresholdRetriever:
    """Return up to k distinct matching documents scoring at least a threshold."""

    def __init__(
        self,
        store: InMemoryVectorStore,
        k: int = 4,
        score_threshold: float = 0.0,
        metadata_filter: Mapping[str, Any] | None = None,
    ) -> None:
        if k < 1:
            raise ValueError("k must be at least 1")
        if not 0.0 <= score_threshold <= 1.0:
            raise ValueError("score_threshold must be within [0, 1]")
        self.store = store
        self.k = k
        self.score_threshold = score_threshold
        self.metadata_filter = dict(metadata_filter or {})

    def matches(self, doc: Document) -> bool:
        """Return whether every filter key equals the document metadata."""
        return all(
            doc.metadata.get(key) == value
            for key, value in self.metadata_filter.items()
        )

    def invoke(self, query: str) -> list[Document]:
        """Filter, dedupe by metadata id, then keep the best k documents."""
        seen: set[Any] = set()
        results: list[Document] = []
        for doc, score in self.store.similarity_search_with_score(query):
            if score < self.score_threshold:
                break
            if not self.matches(doc):
                continue
            doc_id = doc.metadata.get("id", doc.page_content)
            if doc_id in seen:
                continue
            seen.add(doc_id)
            results.append(doc)
            if len(results) == self.k:
                break
        return results
