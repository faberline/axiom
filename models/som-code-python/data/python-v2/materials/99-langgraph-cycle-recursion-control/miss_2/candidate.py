"""ResilientGraphRunner with Router Safeguards and Recursion Bounds."""
from __future__ import annotations

from typing import Any


class GraphRecursionLimitError(RuntimeError):
    """Raised when graph execution exceeds safe recursion limits."""


class ResilientGraphRunner:
    """Cyclic graph workflow with deterministic termination and recursion safety."""

    def __init__(self, max_iterations: int = 5, recursion_limit: int = 25) -> None:
        self.max_iterations = max_iterations
        self.recursion_limit = recursion_limit

    def agent_node(self, state: dict[str, Any]) -> dict[str, Any]:
        task = state.get("task", "")
        iteration = state.get("iteration", 0)
        tool_needed = iteration < 2 and "solve" in task
        return {
            "task": task,
            "iteration": iteration + 1,
            "tool_needed": tool_needed,
            "history": state.get("history", []) + [f"agent_step_{iteration}"],
            "is_complete": state.get("is_complete", False),
        }

    def tool_node(self, state: dict[str, Any]) -> dict[str, Any]:
        history = state.get("history", []) + ["tool_executed"]
        return {
            **state,
            "history": history,
            "tool_needed": False,
        }

    def evaluator_node(self, state: dict[str, Any]) -> dict[str, Any]:
        iteration = state.get("iteration", 0)
        is_complete = iteration >= 2 or not state.get("tool_needed", False)
        return {
            **state,
            "is_complete": is_complete,
            "history": state.get("history", []) + [f"evaluated_complete_{is_complete}"],
        }

    def route_next_step(self, state: dict[str, Any]) -> str:
        if state.get("is_complete", False):
            return "END"
        if state.get("iteration", 0) > self.max_iterations + 5:
            return "END"
        if state.get("tool_needed", False):
            return "tool"
        return "agent"

    def run(self, initial_task: str) -> dict[str, Any]:
        state: dict[str, Any] = {
            "task": initial_task,
            "iteration": 0,
            "tool_needed": False,
            "is_complete": False,
            "history": [],
        }

        current_node = "agent"
        steps_taken = 0

        while current_node != "END":
            steps_taken += 1
            if steps_taken > self.recursion_limit:
                raise GraphRecursionLimitError(f"Recursion limit {self.recursion_limit} exceeded")

            if current_node == "agent":
                state = self.agent_node(state)
                current_node = "tool" if state.get("tool_needed") else "evaluator"
            elif current_node == "tool":
                state = self.tool_node(state)
                current_node = "evaluator"
            elif current_node == "evaluator":
                state = self.evaluator_node(state)
                current_node = self.route_next_step(state)

        return state
