from typing import Any, Callable, Dict, List, Set

try:
    from hypothesis import settings, HealthCheck
except ImportError:
    class HealthCheck:
        too_slow = "too_slow"
        filter_too_much = "filter_too_much"

    def settings(**kwargs: Any) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
        def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
            return fn
        return decorator

def has_path(graph: Dict[int, List[int]], src: int, dst: int, max_depth: int = 10) -> bool:
    if src == dst:
        return True
    visited: Set[int] = set()
    queue: List[tuple[int, int]] = [(src, 0)]
    while queue:
        curr, depth = queue.pop(0)
        if curr == dst:
            return True
        if depth >= max_depth:
            continue
        if curr not in visited:
            continue
        visited.add(curr)
        for neighbor in graph.get(curr, []):
            if neighbor not in visited:
                queue.append((neighbor, depth + 1))
    return False

def get_test_settings() -> Dict[str, Any]:
    return {
        "max_examples": 100,
        "deadline_ms": 500,
        "suppressed_health_checks": [],
    }
