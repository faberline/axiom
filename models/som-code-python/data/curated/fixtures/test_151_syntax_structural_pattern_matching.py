import pytest

from candidate import Move, Say, parse, run


def test_move_with_and_without_steps():
    assert parse("move north 3") == Move(0, 3)
    assert parse("move west") == Move(-1, 0)
    assert parse("move south 2") == Move(0, -2)


def test_whitespace_is_collapsed():
    assert parse("say hello   world") == Say("hello world")
    assert parse("  move  east 2 ") == Move(2, 0)


@pytest.mark.parametrize("line", ["say", "move up 3", "jump", "", "move north 1 2"])
def test_unknown_commands_are_value_errors(line):
    with pytest.raises(ValueError, match="unknown command"):
        parse(line)


@pytest.mark.parametrize("steps", ["0", "x", "-2"])
def test_bad_step_counts_are_value_errors(steps):
    with pytest.raises(ValueError, match="bad step count"):
        parse(f"move north {steps}")


def test_run_accumulates_moves_and_speech():
    lines = ["move north 3", "say hi", "move east 2", "move south", "say bye now"]
    assert run(lines) == ((2, 2), ["hi", "bye now"])
