import hashlib

import pytest
from fastapi.testclient import TestClient

import candidate
from candidate import MAX_BYTES, app

client = TestClient(app)


@pytest.fixture(autouse=True)
def fresh():
    candidate.reset_db()


def upload(name, data, ctype):
    return client.put(f"/avatars/{name}", content=data, headers={"Content-Type": ctype})


def test_png_is_stored_with_digest():
    data = b"\x89PNG" + b"x" * 100
    r = upload("me.png", data, "image/png")
    assert r.status_code == 201
    assert r.json() == {
        "name": "me.png",
        "size": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    }
    assert candidate.STORED["me.png"] == data


def test_content_type_parameters_are_ignored():
    r = upload("a.webp", b"w" * 10, "image/webp; charset=binary")
    assert r.status_code == 201


def test_exactly_the_limit_is_accepted():
    assert upload("big.jpg", b"j" * MAX_BYTES, "image/jpeg").status_code == 201


def test_one_byte_over_the_limit_is_413():
    r = upload("huge.png", b"p" * (MAX_BYTES + 1), "image/png")
    assert r.status_code == 413
    assert r.json() == {"detail": f"body exceeds {MAX_BYTES} bytes"}
    assert candidate.STORED == {}


def test_other_types_are_415():
    r = upload("notes.txt", b"hello", "text/plain")
    assert r.status_code == 415
    assert r.json() == {"detail": "unsupported type text/plain"}
    assert upload("x.png", b"hello", "").status_code == 415


def test_empty_body_is_422():
    r = upload("empty.png", b"", "image/png")
    assert r.status_code == 422
    assert r.json() == {"detail": "body is empty"}
