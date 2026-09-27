import pytest

from candidate import ConsoleErrorCollector


class Message:
    def __init__(self, kind, text):
        self.type = kind
        self.text = text


class Page:
    def __init__(self):
        self.listeners = {}

    def on(self, event, handler):
        self.listeners.setdefault(event, []).append(handler)

    def remove_listener(self, event, handler):
        self.listeners[event].remove(handler)

    def emit(self, event, payload):
        for handler in list(self.listeners.get(event, [])):
            handler(payload)


def test_clean_run_passes_and_detaches_listeners():
    page = Page()
    with ConsoleErrorCollector(page) as collector:
        page.emit("console", Message("log", "hello"))
    assert collector.errors == []
    assert page.listeners == {"console": [], "pageerror": []}


def test_console_errors_fail_the_block():
    page = Page()
    with pytest.raises(AssertionError, match="console errors: boom; bad"):
        with ConsoleErrorCollector(page):
            page.emit("console", Message("error", "boom"))
            page.emit("console", Message("warning", "meh"))
            page.emit("console", Message("error", "bad"))


def test_uncaught_page_errors_are_collected():
    page = Page()
    with pytest.raises(AssertionError, match="TypeError: x is undefined"):
        with ConsoleErrorCollector(page):
            page.emit("pageerror", Exception("TypeError: x is undefined"))


def test_ignore_patterns_match_substrings():
    page = Page()
    with ConsoleErrorCollector(page, ignore=["favicon"]) as collector:
        page.emit("console", Message("error", "GET /favicon.ico 404"))
        page.emit("pageerror", Exception("favicon failed"))
    assert collector.errors == []


def test_events_after_exit_are_not_collected():
    page = Page()
    with ConsoleErrorCollector(page) as collector:
        pass
    page.emit("console", Message("error", "late"))
    assert collector.errors == []


def test_test_failure_is_not_masked():
    page = Page()
    with pytest.raises(KeyError):
        with ConsoleErrorCollector(page):
            page.emit("console", Message("error", "boom"))
            raise KeyError("original")
    assert page.listeners == {"console": [], "pageerror": []}
