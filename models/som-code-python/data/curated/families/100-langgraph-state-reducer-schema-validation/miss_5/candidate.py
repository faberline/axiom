"""Apply validated, immutable node updates to a reducer-annotated graph state."""

from __future__ import annotations

import copy
import operator
from typing import Annotated, Any, TypedDict


class AgentState(TypedDict):
    """Graph state whose messages channel is reduced by concatenation."""

    messages: Annotated[list[str], operator.add]
    metadata: dict[str, Any]
    step_count: int


class ValidatedStateGraph:
    """State graph pipeline enforcing immutable updates and schema validation."""

    def __init__(self) -> None:
        self.schema_keys = set(AgentState.__annotations__.keys())

    def validate_node_output(self, update: dict[str, Any]) -> bool:
        """Whether update names only schema keys and carries list messages."""
        if not isinstance(update, dict):
            return False
        for key in update:
            if key not in self.schema_keys:
                return False
        return "messages" not in update or isinstance(update["messages"], list)

    def ingest_node(self, state: AgentState, incoming_text: str) -> dict[str, Any]:
        """Return an update adding the user's text and one step."""
        return {
            "messages": [f"user: {incoming_text}"],
            "step_count": 1,
        }

    def process_node(self, state: AgentState) -> dict[str, Any]:
        """Return an update marking metadata processed, without mutating state."""
        new_metadata = dict(state.get("metadata", {}))
        new_metadata["processed"] = True
        return {
            "messages": ["system: processed"],
            "metadata": new_metadata,
            "step_count": 1,
        }

    def apply_update(self, state: AgentState, update: dict[str, Any]) -> AgentState:
        """Return a new state with update merged, raising ValueError when invalid."""
        if not self.validate_node_output(update):
            raise ValueError(f"Invalid node update: {update}")

        new_state = copy.deepcopy(state)
        if "messages" in update:
            new_state["messages"] = operator.add(
                new_state["messages"], update["messages"]
            )
        if "metadata" in update:
            new_state["metadata"] = copy.deepcopy(update["metadata"])
        if "step_count" in update:
            new_state["step_count"] = update["step_count"]
        return new_state

    def run_pipeline(self, initial_text: str) -> AgentState:
        """Run ingest then process from initial_text and return the final state."""
        current_state: AgentState = {
            "messages": [],
            "metadata": {"source": "api"},
            "step_count": 0,
        }
        up1 = self.ingest_node(current_state, initial_text)
        current_state = self.apply_update(current_state, up1)

        up2 = self.process_node(current_state)
        current_state = self.apply_update(current_state, up2)
        return current_state
