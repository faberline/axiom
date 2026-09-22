import pytest
from candidate import GraphRecursionLimitError, ResilientGraphRunner


def test_graph_normal_completion_terminates_at_end():
    runner = ResilientGraphRunner(max_iterations=5)
    final_state = runner.run("solve problem")
    assert final_state["is_complete"] is True
    assert final_state["iteration"] <= 5
    assert "evaluated_complete_True" in final_state["history"]


def test_uncompletable_task_terminates_at_max_iterations():
    runner = ResilientGraphRunner(max_iterations=3)
    final_state = runner.run("unresolvable problem")
    assert final_state["iteration"] <= 3, f"Expected iteration <= 3, got {final_state['iteration']}"


def test_route_next_step_returns_end_on_completion():
    runner = ResilientGraphRunner(max_iterations=5)
    step = runner.route_next_step({"is_complete": True, "iteration": 1})
    assert step == "END", f"Expected END on completion, got {step}"


def test_route_next_step_returns_end_on_iteration_cap():
    runner = ResilientGraphRunner(max_iterations=4)
    step = runner.route_next_step({"is_complete": False, "iteration": 4})
    assert step == "END", f"Expected END on iteration cap, got {step}"


def test_recursion_limit_raises_error():
    runner = ResilientGraphRunner(max_iterations=50, recursion_limit=2)
    with pytest.raises(GraphRecursionLimitError):
        runner.run("solve problem")


def test_default_max_iterations_bound():
    runner = ResilientGraphRunner()
    assert runner.max_iterations <= 10, f"Default max_iterations too large: {runner.max_iterations}"
    assert runner.recursion_limit <= 30, f"Default recursion_limit too large: {runner.recursion_limit}"
