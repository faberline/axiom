"""Parse amounts, logging skipped lines through a module-level logger."""

import logging
from collections.abc import Iterable

logger = logging.getLogger(__name__)


def parse_amounts(lines: Iterable[str]) -> list[int]:
    """Return the non-negative integer amounts, warning about each bad line."""
    amounts: list[int] = []
    seen = 0
    for number, line in enumerate(lines):
        text = line.strip()
        if not text:
            continue
        seen += 1
        try:
            value = int(text)
        except ValueError:
            logger.warning("line %d: not an integer: %r", number, text)
            continue
        if value < 0:
            logger.warning("line %d: negative amount %d skipped", number, value)
            continue
        amounts.append(value)
    if not amounts:
        logger.error("no valid amounts in %d lines", seen)
        raise ValueError("no valid amounts in input")
    logger.info("parsed %d of %d lines", len(amounts), seen)
    return amounts
