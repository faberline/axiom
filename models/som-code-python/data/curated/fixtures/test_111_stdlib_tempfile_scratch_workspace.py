import tempfile
from pathlib import Path

import pytest

from candidate import render_bundle, workspace


def test_build_sees_nested_files_and_workspace_is_removed():
    seen = {}

    def build(root):
        seen["root"] = root
        return (root / "src" / "pkg" / "main.txt").read_bytes()

    assert render_bundle({"src/pkg/main.txt": "hello"}, build) == b"hello"
    assert seen["root"].name.startswith("build-")
    assert not seen["root"].exists()


def test_workspace_is_removed_when_the_build_fails():
    seen = {}

    def build(root):
        seen["root"] = root
        raise RuntimeError("compiler crashed")

    with pytest.raises(RuntimeError, match="compiler crashed"):
        render_bundle({"a.txt": "x"}, build)
    assert not seen["root"].exists()


def test_paths_escaping_the_workspace_are_rejected():
    outside = Path(tempfile.gettempdir()) / "escape-111.txt"
    try:
        with pytest.raises(ValueError, match="path escapes workspace"):
            render_bundle({"../escape-111.txt": "bad"}, lambda root: b"")
        assert not outside.exists()
    finally:
        outside.unlink(missing_ok=True)


def test_prefix_must_be_a_plain_name():
    with pytest.raises(ValueError, match="prefix must be a non-empty name"):
        with workspace("a/b"):
            pass
