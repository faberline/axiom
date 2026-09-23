import pytest
from candidate import ValidatedStateGraph


def test_nodes_do_not_mutate_input_state_in_place():
    graph = ValidatedStateGraph()
    state = {"messages": ["initial"], "metadata": {"key": 1}, "step_count": 0}
    original_messages = list(state["messages"])
    original_metadata = dict(state["metadata"])

    _ = graph.ingest_node(state, "hello")
    assert state["messages"] == original_messages, "Input messages list was mutated in-place!"

    _ = graph.process_node(state)
    assert state["metadata"] == original_metadata, "Input metadata dict was mutated in-place!"


def test_reducer_concatenates_messages_sequentially():
    graph = ValidatedStateGraph()
    final_state = graph.run_pipeline("order request")
    assert len(final_state["messages"]) == 2, f"Expected 2 messages, got {len(final_state['messages'])}"
    assert final_state["messages"][0] == "user: order request"
    assert final_state["messages"][1] == "system: processed"


def test_validate_node_output_rejects_raw_string_for_list_reducer():
    graph = ValidatedStateGraph()
    assert graph.validate_node_output({"messages": ["valid list"]}) is True
    assert graph.validate_node_output({"messages": "invalid string"}) is False


def test_validate_node_output_rejects_undeclared_schema_keys():
    graph = ValidatedStateGraph()
    assert graph.validate_node_output({"unregistered_key": "val"}) is False


def test_step_count_increments_monotonically():
    graph = ValidatedStateGraph()
    final_state = graph.run_pipeline("test")
    assert final_state["step_count"] == 2, f"Expected step_count == 2, got {final_state['step_count']}"
