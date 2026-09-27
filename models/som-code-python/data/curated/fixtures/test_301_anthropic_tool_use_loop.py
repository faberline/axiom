from types import SimpleNamespace as NS

import pytest

from candidate import ToolLoopError, run_tool_loop


def text(t):
    return NS(type="text", text=t)


def use(id_, name, **args):
    return NS(type="tool_use", id=id_, name=name, input=args)


class FakeMessages:
    def __init__(self, responses):
        self.responses = list(responses)
        self.requests = []

    def create(self, **kwargs):
        self.requests.append({**kwargs, "messages": list(kwargs["messages"])})
        return self.responses.pop(0)


SCHEMAS = [{"name": "add", "input_schema": {"type": "object"}}]
TOOLS = {"add": lambda a, b: a + b, "lookup": lambda key: {"key": key}}


def test_tool_results_are_sent_back_and_final_text_joined():
    api = FakeMessages(
        [
            NS(
                stop_reason="tool_use",
                content=[
                    text("thinking"),
                    use("t1", "add", a=2, b=3),
                    use("t2", "lookup", key="k"),
                ],
            ),
            NS(stop_reason="end_turn", content=[text("The sum "), text("is 5.")]),
        ]
    )
    assert run_tool_loop(api, "add", TOOLS, SCHEMAS) == "The sum is 5."
    second = api.requests[1]["messages"]
    assert second[0] == {"role": "user", "content": "add"}
    assert second[1]["role"] == "assistant"
    assert second[2] == {
        "role": "user",
        "content": [
            {"type": "tool_result", "tool_use_id": "t1", "content": "5"},
            {"type": "tool_result", "tool_use_id": "t2", "content": '{"key": "k"}'},
        ],
    }
    assert api.requests[0]["tools"] == SCHEMAS


def test_tool_errors_are_reported_to_the_model():
    api = FakeMessages(
        [
            NS(
                stop_reason="tool_use", content=[use("a", "nope"), use("b", "add", a=1)]
            ),
            NS(stop_reason="end_turn", content=[text("sorry")]),
        ]
    )
    assert run_tool_loop(api, "x", TOOLS, SCHEMAS) == "sorry"
    results = api.requests[1]["messages"][2]["content"]
    assert results[0]["is_error"] is True and "nope" in results[0]["content"]
    assert results[1]["is_error"] is True and results[1]["content"].startswith(
        "TypeError"
    )


def test_turn_limit_raises():
    loop = [
        NS(stop_reason="tool_use", content=[use(str(i), "add", a=1, b=1)])
        for i in range(3)
    ]
    with pytest.raises(ToolLoopError):
        run_tool_loop(FakeMessages(loop), "x", TOOLS, SCHEMAS, max_turns=3)


def test_plain_answer_needs_one_request():
    api = FakeMessages([NS(stop_reason="end_turn", content=[text("hi")])])
    assert run_tool_loop(api, "hello", TOOLS, SCHEMAS) == "hi"
    assert len(api.requests) == 1
    assert api.requests[0]["max_tokens"] == 1024
