import json
import os

import pytest

from candidate import authenticated_context


class Context:
    def __init__(self, kwargs):
        self.kwargs = kwargs
        self.closed = False

    def new_page(self):
        return self

    def storage_state(self, path=None):
        state = {"cookies": [{"name": "sid", "value": "1"}]}
        if path is not None:
            with open(path, "w") as fh:
                json.dump(state, fh)
        return state

    def close(self):
        self.closed = True


class Browser:
    def __init__(self):
        self.contexts = []

    def new_context(self, **kwargs):
        context = Context(kwargs)
        self.contexts.append(context)
        return context


def counting_login():
    calls = []

    def login(page):
        calls.append(page)

    return login, calls


def test_first_call_logs_in_and_saves_state(tmp_path):
    path = tmp_path / "state.json"
    browser = Browser()
    login, calls = counting_login()
    context = authenticated_context(browser, path, login)
    assert len(calls) == 1
    assert context.kwargs == {}
    assert json.loads(path.read_text())["cookies"][0]["name"] == "sid"


def test_fresh_state_is_reused_without_login(tmp_path):
    path = tmp_path / "state.json"
    path.write_text("{}")
    os.utime(path, (1000.0, 1000.0))
    browser = Browser()
    login, calls = counting_login()
    context = authenticated_context(browser, path, login, now=lambda: 1000.0 + 3599)
    assert calls == []
    assert context.kwargs == {"storage_state": str(path)}


def test_state_at_max_age_is_refreshed(tmp_path):
    path = tmp_path / "state.json"
    path.write_text("{}")
    os.utime(path, (1000.0, 1000.0))
    login, calls = counting_login()
    authenticated_context(Browser(), path, login, max_age=60, now=lambda: 1060.0)
    assert len(calls) == 1
    assert path.read_text() != "{}"


def test_failed_login_closes_the_context(tmp_path):
    path = tmp_path / "state.json"
    browser = Browser()

    def login(page):
        raise RuntimeError("bad password")

    with pytest.raises(RuntimeError, match="bad password"):
        authenticated_context(browser, path, login)
    assert browser.contexts[0].closed is True
    assert not path.exists()
