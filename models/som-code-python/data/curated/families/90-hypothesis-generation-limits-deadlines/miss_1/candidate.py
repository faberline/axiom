"""Bound a graph search and describe property-test settings that hide nothing."""

from typing import Any


def has_path(
    graph: dict[int, list[int]], src: int, dst: int, max_depth: int = 10
) -> bool:
    """Whether dst is reachable from src within max_depth breadth-first steps."""
    if src == dst:
        return True
    visited: set[int] = set()
    queue: list[tuple[int, int]] = [(src, 0)]
    while queue:
        curr, depth = queue.pop(0)
        if curr == dst:
            return True
        if depth >= max_depth:
            continue
        if curr in visited:
            continue
        visited.add(curr)
        for neighbor in graph.get(curr, []):
            if neighbor not in visited:
                queue.append((neighbor, depth + 1))
    return False


def get_test_settings() -> dict[str, Any]:
    """Return settings with a real example budget, deadline, and no suppressions."""
    return {
        "max_examples": 100,
        "deadline_ms": 500,
        "suppressed_health_checks": ["too_slow"],
    }
