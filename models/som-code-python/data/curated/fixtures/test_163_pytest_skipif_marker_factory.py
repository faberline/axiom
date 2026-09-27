import pytest

from candidate import requires_env, requires_tool


def test_all_variables_present_does_not_skip(monkeypatch):
    monkeypatch.setenv("DB_URL", "sqlite://")
    monkeypatch.setenv("DB_USER", "app")
    marker = requires_env("DB_URL", "DB_USER")
    assert marker.mark.name == "skipif"
    assert marker.mark.args == (False,)


def test_missing_variables_skip_and_are_named(monkeypatch):
    monkeypatch.setenv("DB_URL", "sqlite://")
    monkeypatch.delenv("DB_USER", raising=False)
    monkeypatch.delenv("DB_PASS", raising=False)
    marker = requires_env("DB_URL", "DB_USER", "DB_PASS")
    assert marker.mark.args == (True,)
    assert marker.mark.kwargs["reason"] == "missing environment: DB_USER, DB_PASS"


def test_empty_variable_counts_as_missing(monkeypatch):
    monkeypatch.setenv("DB_URL", "")
    assert requires_env("DB_URL").mark.args == (True,)


def test_environment_is_read_when_called(monkeypatch):
    monkeypatch.delenv("LATE_VAR", raising=False)
    assert requires_env("LATE_VAR").mark.args == (True,)
    monkeypatch.setenv("LATE_VAR", "1")
    assert requires_env("LATE_VAR").mark.args == (False,)


def test_requires_at_least_one_name():
    with pytest.raises(ValueError, match="at least one environment variable"):
        requires_env()


def test_tool_on_path(monkeypatch, tmp_path):
    tool = tmp_path / "frobnicate"
    tool.write_text("#!/bin/sh\n")
    tool.chmod(0o755)
    monkeypatch.setenv("PATH", str(tmp_path))
    assert requires_tool("frobnicate").mark.args == (False,)


def test_tool_missing_from_path(monkeypatch, tmp_path):
    monkeypatch.setenv("PATH", str(tmp_path))
    (tmp_path / "frobnicate").write_text("not executable")
    marker = requires_tool("frobnicate")
    assert marker.mark.name == "skipif"
    assert marker.mark.args == (True,)
    assert marker.mark.kwargs["reason"] == "frobnicate not found on PATH"
