import hashlib

import pytest

from candidate import file_digest, verify_manifest


def test_streamed_digest_matches_one_shot_hash(tmp_path):
    data = b"abcdefghij" * 7
    path = tmp_path / "blob.bin"
    path.write_bytes(data)
    assert file_digest(path, chunk_size=3) == hashlib.sha256(data).hexdigest()
    assert file_digest(path, "sha1") == hashlib.sha1(data).hexdigest()


def test_bad_arguments_are_rejected(tmp_path):
    path = tmp_path / "x.bin"
    path.write_bytes(b"x")
    with pytest.raises(ValueError, match="unsupported algorithm"):
        file_digest(path, "nope")
    with pytest.raises(ValueError, match="chunk_size must be positive"):
        file_digest(path, chunk_size=0)


def test_manifest_reports_changed_and_missing_files(tmp_path):
    (tmp_path / "a.txt").write_bytes(b"alpha")
    (tmp_path / "b.txt").write_bytes(b"beta")
    manifest = {
        "b.txt": hashlib.sha256(b"beta").hexdigest().upper(),
        "a.txt": hashlib.sha256(b"ALPHA").hexdigest(),
        "c.txt": hashlib.sha256(b"gamma").hexdigest(),
    }
    assert verify_manifest(tmp_path, manifest) == ["a.txt", "c.txt"]
