"""ValidatedStateGraph with Immutable Node Updates and Reducer Contracts."""
from __future__ import annotations

import copy
import operator
from typing import Annotated, Any, TypedDict


class AgentState(TypedDict):
    messages: Annotated[list[str], operator.add]
    metadata: dict[str, Any]
    step_count: int


class ValidatedStateGraph:
    """State graph pipeline enforcing immutable updates and schema validation."""

    def __init__(self) -> None:
        self.schema_keys = set(AgentState.__annotations__.keys())

    def validate_node_output(self, update: dict[str, Any]) -> bool:
        if not isinstance(update, dict):
            return False
        for key in update.keys():
            if key not in self.schema_keys:
                return False
        if "messages" in update and not isinstance(update["messages"], list):
            return False
        return True

    def ingest_node(self, state: AgentState, incoming_text: str) -> dict[str, Any]:
        return {
            "messages": [f"user: {incoming_text}"],
            "step_count": 1,
        }

    def process_node(self, state: AgentState) -> dict[str, Any]:
        new_metadata = dict(state.get("metadata", {}))
        new_metadata["processed"] = True
        return {
            "messages": ["system: processed"],
            "metadata": new_metadata,
            "step_count": 1,
        }

    def apply_update(self, state: AgentState, update: dict[str, Any]) -> AgentState:
        if not self.validate_node_output(update):
            raise ValueError(f"Invalid node update: {update}")

        new_state = copy.deepcopy(state)
        if "messages" in update:
            new_state["messages"] = operator.add(new_state["messages"], update["messages"])
        if "metadata" in update:
            new_state["metadata"] = copy.deepcopy(update["metadata"])
        if "step_count" in update:
            new_state["step_count"] = update["step_count"]
        return new_state

    def run_pipeline(self, initial_text: str) -> AgentState:
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
