import pytest

from candidate import FastMCP, ToolError


def make_server():
    mcp = FastMCP("demo")

    @mcp.tool()
    def divide(a: float, b: float = 1.0) -> float:
        """Divide a by b."""
        return a / b

    @mcp.tool(name="add_numbers")
    def add(a: int, b: int) -> int:
        """Add two integers."""
        return a + b

    return mcp, divide


def test_tools_are_listed_by_name_with_docstrings():
    mcp, _ = make_server()
    tools = mcp.list_tools()
    assert [t.name for t in tools] == ["add_numbers", "divide"]
    assert tools[1].description == "Divide a by b."
    assert tools[1].params == ("a", "b")
    assert tools[1].required == ("a",)


def test_decorator_returns_the_original_function():
    _, divide = make_server()
    assert divide(6, 3) == 2


def test_call_uses_defaults_and_custom_names():
    mcp, _ = make_server()
    assert mcp.call_tool("divide", {"a": 5}) == 5
    assert mcp.call_tool("add_numbers", {"a": 2, "b": 3}) == 5


def test_duplicate_names_are_rejected():
    mcp, _ = make_server()
    with pytest.raises(ToolError, match="already"):
        mcp.tool(name="divide")(lambda: None)


def test_malformed_calls_raise_tool_error():
    mcp, _ = make_server()
    with pytest.raises(ToolError, match="unknown"):
        mcp.call_tool("nope", {})
    with pytest.raises(ToolError, match="unexpected arguments: c"):
        mcp.call_tool("divide", {"a": 1, "c": 2})
    with pytest.raises(ToolError, match="missing arguments: a, b"):
        mcp.call_tool("add_numbers", {})


def test_tool_failures_are_wrapped_with_their_cause():
    mcp, _ = make_server()
    with pytest.raises(ToolError, match="failed") as info:
        mcp.call_tool("divide", {"a": 1, "b": 0})
    assert isinstance(info.value.__cause__, ZeroDivisionError)
