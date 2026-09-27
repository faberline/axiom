"""Configure a named logger idempotently so repeated setup never duplicates output."""

import logging
from typing import TextIO

LOG_FORMAT = "%(levelname)s %(name)s: %(message)s"
HANDLER_NAME = "app-stream"


def configure_logger(
    name: str, stream: TextIO, level: int | str = logging.INFO
) -> logging.Logger:
    """Attach exactly one formatted stream handler to the named logger."""
    if isinstance(level, str):
        resolved = logging.getLevelNamesMapping().get(level.upper())
        if resolved is None:
            raise ValueError(f"unknown log level: {level}")
    else:
        resolved = level
    logger = logging.getLogger(name)
    for existing in list(logger.handlers):
        if existing.get_name() != HANDLER_NAME:
            logger.removeHandler(existing)
            existing.close()
    handler = logging.StreamHandler(stream)
    handler.set_name(HANDLER_NAME)
    handler.setFormatter(logging.Formatter(LOG_FORMAT))
    logger.addHandler(handler)
    logger.setLevel(resolved)
    logger.propagate = False
    return logger
