from types import SimpleNamespace as NS

import pytest

from candidate import BudgetError, fit_history


class WordCounter:
    def __init__(self):
        self.calls = []

    def count_tokens(self, **kwargs):
        self.calls.append(kwargs)
        words = len(kwargs["system"].split())
        for message in kwargs["messages"]:
            words += len(str(message["content"]).split())
        return NS(input_tokens=words)


def user(text):
    return {"role": "user", "content": text}


def bot(text):
    return {"role": "assistant", "content": text}


TOOL_TURN = [
    user("one two three"),
    {"role": "assistant", "content": [{"type": "tool_use", "id": "t", "name": "x"}]},
    {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t"}]},
    bot("four five"),
    user("six"),
]


def test_everything_fits():
    history = [user("a b"), bot("c"), user("d")]
    assert fit_history(WordCounter(), "sys", history, budget=300) == history


def test_exact_fit_is_accepted():
    history = [user("a b"), bot("c"), user("d")]
    assert fit_history(WordCounter(), "sys", history, budget=10, reserve=5) == history
    assert fit_history(WordCounter(), "sys", history, budget=9, reserve=4) == history


def test_oldest_turns_are_dropped_and_reserve_is_honoured():
    history = [user("a b c"), bot("d e"), user("f"), bot("g"), user("h")]
    out = fit_history(WordCounter(), "s", history, budget=10, reserve=5)
    assert out == [user("f"), bot("g"), user("h")]


def test_tool_result_never_starts_the_window():
    counter = WordCounter()
    out = fit_history(counter, "s", TOOL_TURN, budget=12, reserve=3)
    assert out[0] == user("six")
    assert all(call["model"] == "claude-sonnet-5" for call in counter.calls)


def test_budget_errors():
    counter = WordCounter()
    with pytest.raises(BudgetError):
        fit_history(counter, "s", [user("a b c d e f")], budget=6, reserve=2)
    with pytest.raises(BudgetError):
        fit_history(counter, "s", [user("a"), bot("b")], budget=100, reserve=10)
    with pytest.raises(BudgetError):
        fit_history(counter, "s", [], budget=100, reserve=10)
    with pytest.raises(BudgetError):
        fit_history(counter, "s", [user("a")], budget=5, reserve=5)
    assert issubclass(BudgetError, ValueError)
