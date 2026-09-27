import io
import logging

import pytest

from candidate import configure_logger


class Collect(logging.Handler):
    def __init__(self):
        super().__init__()
        self.records = []

    def emit(self, record):
        self.records.append(record)


def test_reconfiguring_replaces_the_handler_instead_of_adding_one():
    first, second = io.StringIO(), io.StringIO()
    configure_logger("t108.replace", first)
    logger = configure_logger("t108.replace", second)
    logger.info("hello")
    assert second.getvalue() == "INFO t108.replace: hello\n"
    assert first.getvalue() == ""


def test_foreign_handlers_survive_reconfiguration():
    foreign = Collect()
    logging.getLogger("t108.foreign").addHandler(foreign)
    configure_logger("t108.foreign", io.StringIO())
    logger = configure_logger("t108.foreign", io.StringIO())
    logger.warning("kept")
    assert [r.getMessage() for r in foreign.records] == ["kept"]


def test_default_level_is_info():
    out = io.StringIO()
    logger = configure_logger("t108.default", out)
    logger.debug("hidden")
    logger.info("shown")
    assert out.getvalue() == "INFO t108.default: shown\n"


def test_level_names_are_case_insensitive():
    out = io.StringIO()
    configure_logger("t108.names", out, "debug").debug("detail")
    assert out.getvalue() == "DEBUG t108.names: detail\n"


def test_unknown_level_name_is_rejected():
    with pytest.raises(ValueError, match="unknown log level: loud"):
        configure_logger("t108.unknown", io.StringIO(), "loud")


def test_records_do_not_propagate_to_root():
    root = Collect()
    logging.getLogger().addHandler(root)
    try:
        configure_logger("t108.propagate", io.StringIO()).error("once")
    finally:
        logging.getLogger().removeHandler(root)
    assert root.records == []
