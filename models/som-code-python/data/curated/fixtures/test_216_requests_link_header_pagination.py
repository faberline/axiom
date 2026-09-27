import json
import threading
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlsplit

import pytest
import requests

from candidate import PaginationError, fetch_all

TOTAL = 5


def make_handler(seen):
    class Handler(BaseHTTPRequestHandler):
        def reply(self, status, body, link=None):
            data = json.dumps(body).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            if link:
                base = f"http://127.0.0.1:{self.server.server_address[1]}"
                self.send_header("Link", f'<{base}{link}>; rel="next"')
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            parts = urlsplit(self.path)
            query = parse_qs(parts.query)
            seen.append(self.path)
            if any(len(v) > 1 for v in query.values()):
                self.reply(400, {"error": "repeated parameter"})
                return
            page = int(query.get("page", ["1"])[0])
            if parts.path == "/items":
                size = int(query["per_page"][0])
                chunk = list(range(1, TOTAL + 1))[(page - 1) * size : page * size]
                more = page * size < TOTAL
                link = f"/items?per_page={size}&page={page + 1}" if more else None
                self.reply(200, chunk, link)
            elif parts.path == "/pages":
                last = int(query["last"][0])
                link = f"/pages?last={last}&page={page + 1}" if page < last else None
                self.reply(200, [page], link)
            elif parts.path == "/loop":
                self.reply(200, [page], f"/loop?page={page + 1}")
            elif parts.path == "/object":
                self.reply(200, {"items": [1, 2]})
            else:
                self.reply(500, {"error": "boom"})

        def log_message(self, *args):
            pass

    return Handler


@contextmanager
def server():
    seen = []
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(seen))
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    try:
        yield f"http://127.0.0.1:{httpd.server_address[1]}", seen
    finally:
        httpd.shutdown()
        httpd.server_close()


def test_pages_are_followed_without_repeating_params():
    with server() as (base, seen), requests.Session() as s:
        items = fetch_all(s, base + "/items", params={"per_page": "2"})
    assert items == [1, 2, 3, 4, 5]
    assert len(seen) == 3


def test_exactly_max_pages_is_allowed():
    with server() as (base, seen), requests.Session() as s:
        items = fetch_all(s, base + "/pages", params={"last": "3"}, max_pages=3)
    assert items == [1, 2, 3]


def test_endless_listing_raises_after_default_budget():
    with server() as (base, seen), requests.Session() as s:
        with pytest.raises(PaginationError):
            fetch_all(s, base + "/loop")
    assert len(seen) == 50


def test_non_list_page_raises():
    with server() as (base, _), requests.Session() as s:
        with pytest.raises(PaginationError):
            fetch_all(s, base + "/object")


def test_http_errors_raise():
    with server() as (base, _), requests.Session() as s:
        with pytest.raises(requests.HTTPError):
            fetch_all(s, base + "/broken")
