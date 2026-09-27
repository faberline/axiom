"""Parse and run robot commands with structural pattern matching."""

from collections.abc import Iterable
from dataclasses import dataclass

DIRECTIONS: dict[str, tuple[int, int]] = {
    "north": (0, 1),
    "south": (0, -1),
    "east": (1, 0),
    "west": (-1, 0),
}


@dataclass(frozen=True)
class Move:
    """Shift the robot by dx and dy."""

    dx: int
    dy: int


@dataclass(frozen=True)
class Say:
    """Make the robot speak a line of text."""

    text: str


def parse(line: str) -> Move | Say:
    """Turn one command line into a Move or a Say."""
    match line.split(" "):
        case ["move", direction] if direction in DIRECTIONS:
            dx, dy = DIRECTIONS[direction]
            return Move(dx, dy)
        case ["move", direction, steps] if direction in DIRECTIONS:
            if not steps.isdigit() or int(steps) < 1:
                raise ValueError(f"bad step count: {steps}")
            dx, dy = DIRECTIONS[direction]
            return Move(dx * int(steps), dy * int(steps))
        case ["say", *words] if words:
            return Say(" ".join(words))
        case _:
            raise ValueError(f"unknown command: {line!r}")


def run(lines: Iterable[str]) -> tuple[tuple[int, int], list[str]]:
    """Run commands from the origin; return the final position and spoken text."""
    x = y = 0
    spoken: list[str] = []
    for line in lines:
        match parse(line):
            case Move(dx=dx, dy=dy):
                x, y = x + dx, y + dy
            case Say(text=text):
                spoken.append(text)
    return (x, y), spoken
