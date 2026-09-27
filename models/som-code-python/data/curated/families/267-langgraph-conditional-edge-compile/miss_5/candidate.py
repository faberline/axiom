"""A LangGraph-style graph builder that validates edges when compiled."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

State = dict[str, Any]
Node = Callable[[State], State]
Router = Callable[[State], str]
START = "__start__"
END = "__end__"


class GraphError(ValueError):
    """Raised when the graph structure is invalid."""


class GraphRecursionError(RuntimeError):
    """Raised when a run takes more steps than the recursion limit."""


class StateGraph:
    """Collect nodes and edges, then compile them into a runnable graph."""

    def __init__(self) -> None:
        self.nodes: dict[str, Node] = {}
        self.edges: dict[str, str] = {}
        self.branches: dict[str, tuple[Router, dict[str, str]]] = {}

    def add_node(self, name: str, node: Node) -> None:
        """Register a node under a unique, non-reserved name."""
        if name in (START, END) or name in self.nodes:
            raise GraphError(f"invalid or duplicate node name {name!r}")
        self.nodes[name] = node

    def add_edge(self, source: str, target: str) -> None:
        """Add an unconditional edge."""
        self.edges[source] = target

    def add_conditional_edges(
        self, source: str, router: Router, path_map: dict[str, str]
    ) -> None:
        """Route from source to path_map[router(state)]."""
        self.branches[source] = (router, dict(path_map))

    def compile(self) -> CompiledGraph:
        """Check every edge and return a runnable graph."""
        known = set(self.nodes) | {START, END}
        targets = list(self.edges.items())
        for source, (_, path_map) in self.branches.items():
            targets.extend((source, target) for target in path_map.values())
        for source, target in targets:
            if source not in known or target not in known or source == END:
                raise GraphError(f"edge {source!r} -> {target!r} is invalid")
        if START not in self.edges:
            raise GraphError("graph has no entry edge from START")
        return CompiledGraph(self)


class CompiledGraph:
    """A validated graph that merges node updates into the state."""

    def __init__(self, graph: StateGraph) -> None:
        self._nodes = dict(graph.nodes)
        self._edges = dict(graph.edges)
        self._branches = dict(graph.branches)

    def _next(self, current: str, state: State) -> str:
        if current in self._branches:
            router, path_map = self._branches[current]
            key = router(state)
            if key not in path_map:
                raise GraphError(f"router of {current!r} returned unknown {key!r}")
            return path_map[key]
        return self._edges[current]

    def invoke(self, state: State, recursion_limit: int = 25) -> State:
        """Run from START to END and return the final state."""
        current = self._edges[START]
        state = dict(state)
        steps = 0
        while current != END:
            steps += 1
            if steps > recursion_limit:
                raise GraphRecursionError(f"exceeded {recursion_limit} steps")
            state = {**state, **self._nodes[current](state)}
            current = self._next(current, state)
        return state
