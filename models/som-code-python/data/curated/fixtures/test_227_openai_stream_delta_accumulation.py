import json

import pytest

from candidate import IncompleteStreamError, accumulate


def chunk(delta, finish=None, index=0):
    return {"choices": [{"index": index, "delta": delta, "finish_reason": finish}]}


def tool(index, call_id=None, name=None, arguments=None):
    function = {}
    if name is not None:
        function["name"] = name
    if arguments is not None:
        function["arguments"] = arguments
    return {"tool_calls": [{"index": index, "id": call_id, "function": function}]}


def test_content_deltas_are_concatenated():
    message = accumulate(
        [
            chunk({"role": "assistant", "content": None}),
            chunk({"content": "Hel"}),
            chunk({"content": "lo"}),
            chunk({}, finish="stop"),
            {"choices": [], "usage": {"total_tokens": 9}},
        ]
    )
    assert message.content == "Hello"
    assert message.finish_reason == "stop"
    assert message.tool_calls == []


def test_interleaved_tool_calls_are_assembled_by_index():
    message = accumulate(
        [
            chunk(tool(1, "call_b", "lookup", "")),
            chunk(tool(0, "call_a", "search", '{"q": ')),
            chunk(tool(1, arguments='{"id": 7}')),
            chunk(tool(0, arguments='"cats"}')),
            chunk({}, finish="tool_calls"),
        ]
    )
    assert [c.id for c in message.tool_calls] == ["call_a", "call_b"]
    assert [c.name for c in message.tool_calls] == ["search", "lookup"]
    assert json.loads(message.tool_calls[0].arguments) == {"q": "cats"}
    assert json.loads(message.tool_calls[1].arguments) == {"id": 7}


def test_other_choices_are_ignored():
    message = accumulate(
        [
            chunk({"content": "yes"}),
            chunk({"content": "NO"}, index=1),
            chunk({}, finish="stop"),
            chunk({}, finish="length", index=1),
        ]
    )
    assert message.content == "yes"
    assert message.finish_reason == "stop"


def test_stream_without_finish_reason_is_incomplete():
    with pytest.raises(IncompleteStreamError):
        accumulate([chunk({"content": "partial"})])
