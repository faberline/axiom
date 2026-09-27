import hashlib
import os
import threading
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest
import requests

from candidate import ChecksumMismatchError, download

PAYLOAD = bytes(range(256)) * 700
GOOD = hashlib.sha256(PAYLOAD).hexdigest()


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path != "/file":
            self.send_response(404)
            self.send_header("Content-Length", "9")
            self.end_headers()
            self.wfile.write(b"not found")
            return
        self.send_response(200)
        self.send_header("Content-Length", str(len(PAYLOAD)))
        self.end_headers()
        self.wfile.write(PAYLOAD)

    def log_message(self, *args):
        pass


@contextmanager
def server():
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=httpd.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{httpd.server_address[1]}"
    finally:
        httpd.shutdown()
        httpd.server_close()


def fetch(tmp_path, path="/file", sha=GOOD):
    dest = tmp_path / "out.bin"
    with server() as base, requests.Session() as session:
        return download(session, base + path, dest, sha256=sha), dest


def test_matching_download_lands_atomically(tmp_path):
    size, dest = fetch(tmp_path)
    assert size == len(PAYLOAD)
    assert dest.read_bytes() == PAYLOAD
    assert os.listdir(tmp_path) == ["out.bin"]


def test_uppercase_digest_is_accepted(tmp_path):
    _, dest = fetch(tmp_path, sha=GOOD.upper())
    assert dest.read_bytes() == PAYLOAD


def test_mismatch_leaves_no_file(tmp_path):
    with pytest.raises(ChecksumMismatchError):
        fetch(tmp_path, sha="0" * 64)
    assert os.listdir(tmp_path) == []


def test_mismatch_keeps_the_previous_file(tmp_path):
    (tmp_path / "out.bin").write_bytes(b"old")
    with pytest.raises(ChecksumMismatchError):
        fetch(tmp_path, sha="0" * 64)
    assert (tmp_path / "out.bin").read_bytes() == b"old"
    assert os.listdir(tmp_path) == ["out.bin"]


def test_http_error_raises_before_writing(tmp_path):
    with pytest.raises(requests.HTTPError):
        fetch(tmp_path, path="/missing")
    assert os.listdir(tmp_path) == []
