import json
import threading
from contextlib import contextmanager
from email.parser import BytesParser
from email.policy import HTTP
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest
import requests

import candidate
from candidate import UploadTooLargeError, upload

PAYLOAD = b"id,name\n1,caf\xe9\n\xff\xfe\n"


def make_handler(seen):
    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            body = self.rfile.read(int(self.headers["Content-Length"]))
            seen.append(self.path)
            head = f"Content-Type: {self.headers['Content-Type']}\r\n\r\n".encode()
            message = BytesParser(policy=HTTP).parsebytes(head + body)
            parts = {}
            for part in message.iter_parts():
                data = part.get_payload(decode=True)
                parts[part.get_param("name", header="content-disposition")] = {
                    "filename": part.get_filename(),
                    "content_type": part.get_content_type(),
                    "data": data.decode("latin-1"),
                }
            status = 500 if self.path == "/broken" else 201
            reply = json.dumps({"parts": parts} if status == 201 else {"e": 1})
            out = reply.encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(out)))
            self.end_headers()
            self.wfile.write(out)

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


def send(tmp_path, route="/upload", data=PAYLOAD, **kwargs):
    path = tmp_path / "report.csv"
    path.write_bytes(data)
    with server() as (base, seen), requests.Session() as s:
        return upload(s, base + route, path, description="Q3", **kwargs), seen


def test_file_and_description_are_sent_as_parts(tmp_path):
    result, _ = send(tmp_path, content_type="text/csv")
    parts = result["parts"]
    assert parts["file"]["filename"] == "report.csv"
    assert parts["file"]["content_type"] == "text/csv"
    assert parts["file"]["data"].encode("latin-1") == PAYLOAD
    assert parts["description"]["filename"] is None
    assert parts["description"]["data"] == "Q3"


def test_default_content_type_is_octet_stream(tmp_path):
    result, _ = send(tmp_path)
    assert result["parts"]["file"]["content_type"] == "application/octet-stream"


def test_file_at_the_limit_is_sent(tmp_path, monkeypatch):
    monkeypatch.setattr(candidate, "MAX_UPLOAD_BYTES", len(PAYLOAD))
    result, seen = send(tmp_path)
    assert seen == ["/upload"]
    assert "file" in result["parts"]


def test_oversized_file_is_rejected_before_sending(tmp_path, monkeypatch):
    monkeypatch.setattr(candidate, "MAX_UPLOAD_BYTES", 10)
    path = tmp_path / "big.bin"
    path.write_bytes(b"x" * 11)
    with server() as (base, seen), requests.Session() as s:
        with pytest.raises(UploadTooLargeError) as info:
            upload(s, base + "/upload", path, description="d")
    assert (info.value.size, info.value.limit) == (11, 10)
    assert seen == []


def test_default_limit_is_five_mib():
    assert candidate.MAX_UPLOAD_BYTES == 5 * 1024 * 1024


def test_server_errors_raise(tmp_path):
    with pytest.raises(requests.HTTPError):
        send(tmp_path, route="/broken")
