import threading
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import requests

from candidate import TokenAuth


def make_handler(seen, valid):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            auth = self.headers.get("Authorization")
            seen.append((self.path, auth))
            if self.path == "/forbidden":
                status = 403
            else:
                status = 200 if auth in valid else 401
            body = b"ok" if status == 200 else b"no"
            self.send_response(status)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):
            pass

    return Handler


@contextmanager
def server(valid):
    seen = []
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(seen, valid))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    try:
        yield f"http://127.0.0.1:{httpd.server_address[1]}", seen
    finally:
        httpd.shutdown()
        httpd.server_close()


def provider(tokens):
    calls = []

    def fetch():
        calls.append(1)
        return tokens[len(calls) - 1]

    return fetch, calls


def test_token_is_fetched_once_and_reused():
    fetch, calls = provider(["good"])
    with server({"Bearer good"}) as (base, seen), requests.Session() as s:
        s.auth = TokenAuth(fetch)
        assert s.get(base + "/me").status_code == 200
        assert s.get(base + "/me").status_code == 200
    assert len(calls) == 1
    assert [auth for _, auth in seen] == ["Bearer good", "Bearer good"]


def test_expired_token_is_refreshed_once():
    fetch, calls = provider(["stale", "fresh"])
    with server({"Bearer fresh"}) as (base, seen), requests.Session() as s:
        s.auth = TokenAuth(fetch)
        resp = s.get(base + "/me")
    assert resp.status_code == 200
    assert resp.text == "ok"
    assert [r.status_code for r in resp.history] == [401]
    assert [auth for _, auth in seen] == ["Bearer stale", "Bearer fresh"]
    assert len(calls) == 2


def test_persistent_401_is_returned_after_one_retry():
    fetch, calls = provider(["a", "b", "c"])
    with server(set()) as (base, seen), requests.Session() as s:
        s.auth = TokenAuth(fetch)
        resp = s.get(base + "/me")
    assert resp.status_code == 401
    assert len(seen) == 2
    assert len(calls) == 2


def test_other_errors_are_not_retried():
    fetch, calls = provider(["good", "other"])
    with server({"Bearer good"}) as (base, seen), requests.Session() as s:
        s.auth = TokenAuth(fetch)
        resp = s.get(base + "/forbidden")
    assert resp.status_code == 403
    assert len(seen) == 1
    assert len(calls) == 1
