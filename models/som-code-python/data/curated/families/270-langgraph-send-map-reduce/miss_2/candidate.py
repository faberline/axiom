"""A LangGraph-style map-reduce step that fans out with Send objects."""

from __future__ import annotations

import operator
from collections.abc import Callable
from dataclasses import dataclass
from functools import reduce
from typing import Any

MAX_FANOUT = 32


@dataclass(frozen=True)
class Send:
    """Dispatch one private payload to a named node."""

    node: str
    arg: dict[str, Any]


def fan_out(state: dict[str, Any]) -> list[Send]:
    """Return one Send per document, carrying only that document."""
    documents = state["documents"]
    if len(documents) >= MAX_FANOUT:
        raise ValueError(f"at most {MAX_FANOUT} documents per run")
    return [
        Send("summarize", {"doc": doc, "index": index})
        for index, doc in enumerate(documents)
    ]


class MapReduceGraph:
    """Run each Send, then merge the partial summaries with operator.add."""

    def __init__(self, summarize: Callable[[str], str]) -> None:
        self._summarize = summarize

    def _worker(self, arg: dict[str, Any]) -> dict[str, list[tuple[int, str]]]:
        return {"summaries": [(arg["index"], self._summarize(arg["doc"]))]}

    def invoke(self, state: dict[str, Any]) -> dict[str, Any]:
        """Fan out, reduce the worker updates in document order, and join."""
        sends = fan_out(state)
        for send in sends:
            if send.node != "summarize":
                raise ValueError(f"unknown node {send.node!r}")
        updates = [self._worker(send.arg)["summaries"] for send in sends]
        merged: list[tuple[int, str]] = reduce(operator.add, updates, [])
        ordered = [text for _, text in sorted(merged)]
        return {
            **state,
            "summaries": ordered,
            "final": "\n".join(ordered) if ordered else "(no documents)",
        }

    def payload_keys(self, state: dict[str, Any]) -> list[set[str]]:
        """Return the keys each worker receives, for isolation checks."""
        return [set(send.arg) for send in fan_out(state)]

    def worker_count(self, state: dict[str, Any]) -> int:
        """Return how many workers a run would start."""
        return len(fan_out(state))
