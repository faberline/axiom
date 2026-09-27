import pytest

from candidate import RequestBlocker


class Request:
    def __init__(self, url, resource_type="document"):
        self.url = url
        self.resource_type = resource_type


class Route:
    def __init__(self, url, resource_type="document"):
        self.request = Request(url, resource_type)
        self.actions = []

    def abort(self):
        self.actions.append("abort")

    def continue_(self):
        self.actions.append("continue")


def run(blocker, url, kind="document"):
    route = Route(url, kind)
    blocker.handle(route)
    return route.actions


def test_first_party_documents_continue():
    blocker = RequestBlocker(["Example.com"])
    assert run(blocker, "https://example.com/") == ["continue"]
    assert run(blocker, "https://api.example.com/v1", "xhr") == ["continue"]
    assert blocker.blocked == []


def test_host_with_port_and_case_is_matched():
    blocker = RequestBlocker(["example.com"])
    assert run(blocker, "http://EXAMPLE.com:8080/app.js", "script") == ["continue"]


@pytest.mark.parametrize("kind", ["image", "font", "media"])
def test_heavy_resource_types_are_aborted(kind):
    blocker = RequestBlocker(["example.com"])
    assert run(blocker, "https://example.com/x", kind) == ["abort"]


def test_third_party_and_lookalike_hosts_are_aborted():
    blocker = RequestBlocker(["example.com"])
    assert run(blocker, "https://tracker.net/t.js", "script") == ["abort"]
    assert run(blocker, "https://badexample.com/", "document") == ["abort"]
    assert blocker.blocked == ["https://tracker.net/t.js", "https://badexample.com/"]
    blocker.reset()
    assert blocker.blocked == []


def test_allow_list_must_not_be_empty():
    with pytest.raises(ValueError, match="allowed_hosts"):
        RequestBlocker([])
