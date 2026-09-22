import pytest
from pydantic import BaseModel, Field
import candidate
from candidate import OpenAIToolDispatcher, ToolExecutionResult


@pytest.fixture(autouse=True)
def enforce_model_validate(monkeypatch):
    monkeypatch.delattr(BaseModel, "validate", raising=False)
class CalculateArgs(BaseModel):
    x: int
    y: int
    operation: str = Field(default="add")


def calculate(x: int, y: int, operation: str = "add") -> int:
    if operation == "add":
        return x + y
    if operation == "multiply":
        return x * y
    raise ValueError(f"Unknown operation: {operation}")


def no_arg_ping() -> str:
    return "pong"


def test_tool_dispatch_success_with_schema():
    dispatcher = OpenAIToolDispatcher()
    dispatcher.register_tool("calculate", calculate, CalculateArgs)

    call = {
        "id": "call_123",
        "function": {
            "name": "calculate",
            "arguments": '{"x": 10, "y": 20, "operation": "multiply"}',
        },
    }
    result = dispatcher.execute_tool_call(call)
    assert result.status == "success", f"Expected success, got {result.status}: {result.error}"
    assert result.output == 200
    assert result.call_id == "call_123"
    assert result.name == "calculate"


def test_schema_validation_failure_captured():
    dispatcher = OpenAIToolDispatcher()
    dispatcher.register_tool("calculate", calculate, CalculateArgs)

    call = {
        "id": "call_456",
        "function": {
            "name": "calculate",
            "arguments": '{"x": "not_an_int", "y": 20}',
        },
    }
    result = dispatcher.execute_tool_call(call)
    assert result.status == "error", "Expected validation error for non-integer x"
    assert "Argument validation failed" in (result.error or "")


def test_unregistered_tool_error():
    dispatcher = OpenAIToolDispatcher()
    call = {
        "id": "call_789",
        "function": {
            "name": "non_existent_tool",
            "arguments": '{"a": 1}',
        },
    }
    result = dispatcher.execute_tool_call(call)
    assert result.status == "error", "Expected error status for unregistered tool"
    assert "Unrecognized tool" in (result.error or "")


def test_default_empty_arguments_handling():
    dispatcher = OpenAIToolDispatcher()
    dispatcher.register_tool("ping", no_arg_ping)

    call = {
        "id": "call_ping",
        "function": {
            "name": "ping",
        },
    }
    result = dispatcher.execute_tool_call(call)
    assert result.status == "success", f"Expected success with omitted arguments, got: {result.error}"
    assert result.output == "pong"


def test_zero_argument_tool_boundary():
    dispatcher = OpenAIToolDispatcher()
    dispatcher.register_tool("ping", no_arg_ping)

    call = {
        "id": "call_ping_empty",
        "function": {
            "name": "ping",
            "arguments": "{}",
        },
    }
    result = dispatcher.execute_tool_call(call)
    assert result.status == "success", f"Expected success for empty dict arguments, got: {result.error}"
    assert result.output == "pong"


def test_malformed_json_arguments():
    dispatcher = OpenAIToolDispatcher()
    dispatcher.register_tool("calculate", calculate, CalculateArgs)

    call = {
        "id": "call_corrupted",
        "function": {
            "name": "calculate",
            "arguments": '{"x": 10, "y": ',
        },
    }
    result = dispatcher.execute_tool_call(call)
    assert result.status == "error", "Expected error for malformed JSON string"
    assert "Malformed JSON arguments" in (result.error or "")
