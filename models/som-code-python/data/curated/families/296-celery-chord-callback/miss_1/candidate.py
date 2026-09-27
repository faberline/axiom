"""Fan word counting out over Celery tasks and merge them in a chord callback."""

from __future__ import annotations

import re
from collections import Counter
from typing import Any

from celery import Celery, chord

app = Celery("wordcount")
app.conf.update(
    task_always_eager=True,
    task_eager_propagates=True,
    broker_url="memory://",
    result_backend="cache+memory://",
)

WORD = re.compile(r"[a-z']+")


@app.task
def count_words(text: str) -> dict[str, int]:
    """Count the lower-cased words in one document."""
    return dict(Counter(WORD.findall(text.lower())))


@app.task
def merge_counts(results: list[dict[str, int]], top: int) -> list[list[Any]]:
    """Sum the per-document counts and return the top words, ties alphabetical."""
    total: Counter[str] = Counter()
    for counts in results:
        total = Counter(counts)
    ranked = sorted(total.items(), key=lambda item: (-item[1], item[0]))
    return [[word, count] for word, count in ranked[:top]]


def word_report(docs: list[str], top: int = 3) -> Any:
    """Return a chord that counts each document in parallel, then merges."""
    if not docs:
        raise ValueError("at least one document is required")
    if top < 1:
        raise ValueError("top must be at least 1")
    header = [count_words.s(doc) for doc in docs]
    return chord(header, merge_counts.s(top=top))
