import json
import threading
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest
import requests

from candidate import EtagCache


def make_handler(state, seen):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            seen.append((self.path, self.headers.get("If-None-Match")))
            if state.get("fail"):
                self.reply(500, {"error": "down"}, None)
                return
            version = state["versions"].get(self.path, 1)
            etag = f'"v{version}"' if state.get("etag", True) else None
            if etag and self.headers.get("If-None-Match") == etag:
                self.send_response(304)
                self.send_header("ETag", etag)
                self.end_headers()
                return
            self.reply(200, {"path": self.path, "version": version}, etag)

        def reply(self, status, body, etag):
            data = json.dumps(body).encode()
            self.send_response(status)
            if etag:
                self.send_header("ETag", etag)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def log_message(self, *args):
            pass

    return Handler


@contextmanager
def server():
    state = {"versions": {}}
    seen = []
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(state, seen))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    try:
        with requests.Session() as s:
            base = f"http://127.0.0.1:{httpd.server_address[1]}"
            yield base, EtagCache(s), state, seen
    finally:
        httpd.shutdown()
        httpd.server_close()


def test_unchanged_resource_is_served_from_cache():
    with server() as (base, cache, _, seen):
        first = cache.get_json(base + "/a")
        second = cache.get_json(base + "/a")
    assert first == second == {"path": "/a", "version": 1}
    assert seen == [("/a", None), ("/a", '"v1"')]
    assert cache.revalidated == 1


def test_changed_resource_replaces_the_entry():
    with server() as (base, cache, state, seen):
        cache.get_json(base + "/a")
        state["versions"]["/a"] = 2
        assert cache.get_json(base + "/a")["version"] == 2
        assert cache.get_json(base + "/a")["version"] == 2
    assert [inm for _, inm in seen] == [None, '"v1"', '"v2"']
    assert cache.revalidated == 1


def test_urls_are_cached_separately():
    with server() as (base, cache, _, seen):
        cache.get_json(base + "/a")
        cache.get_json(base + "/b")
    assert seen == [("/a", None), ("/b", None)]


def test_response_without_etag_drops_the_entry():
    with server() as (base, cache, state, seen):
        cache.get_json(base + "/a")
        state["etag"] = False
        state["versions"]["/a"] = 2
        cache.get_json(base + "/a")
        cache.get_json(base + "/a")
    assert [inm for _, inm in seen] == [None, '"v1"', None]


def test_errors_raise_and_keep_the_entry():
    with server() as (base, cache, state, seen):
        cache.get_json(base + "/a")
        state["fail"] = True
        with pytest.raises(requests.HTTPError):
            cache.get_json(base + "/a")
        state["fail"] = False
        assert cache.get_json(base + "/a")["version"] == 1
    assert [inm for _, inm in seen] == [None, '"v1"', '"v1"']
