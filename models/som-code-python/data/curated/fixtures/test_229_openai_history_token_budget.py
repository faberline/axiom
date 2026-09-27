import pytest

from candidate import BudgetTooSmallError, trim_history


def words(text):
    return len(text.split())


def msg(role, content):
    return {"role": role, "content": content}


SYSTEM = msg("system", "be brief")
HISTORY = [
    SYSTEM,
    msg("user", "hi there"),
    msg("assistant", " ".join(["word"] * 20)),
    msg("user", "and then"),
    msg("assistant", "sure thing"),
    msg("user", "thanks bye"),
]


def contents(result):
    return [m["content"] for m in result]


def test_newest_turns_that_fit_are_kept_in_order():
    result = trim_history(HISTORY, 30, words)
    assert contents(result) == ["be brief", "and then", "sure thing", "thanks bye"]


def test_history_is_not_resumed_after_a_turn_that_does_not_fit():
    result = trim_history(HISTORY, 36, words)
    assert "hi there" not in contents(result)


def test_exact_budget_is_allowed():
    result = trim_history(HISTORY, 24, words)
    assert contents(result) == ["be brief", "and then", "sure thing", "thanks bye"]


def test_overhead_is_counted_per_message():
    result = trim_history(HISTORY, 23, words)
    assert contents(result) == ["be brief", "sure thing", "thanks bye"]


def test_budget_below_system_plus_last_turn_raises():
    with pytest.raises(BudgetTooSmallError, match="need 12 tokens"):
        trim_history(HISTORY, 11, words)


def test_input_is_not_modified():
    before = list(HISTORY)
    trim_history(HISTORY, 12, words)
    assert HISTORY == before
