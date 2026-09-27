"""Fixture for 178: file-reading code tested against per-path mock_open handles."""

import pytest

from candidate import fake_files, read_settings


def test_parses_pairs_skipping_blanks_and_comments() -> None:
    with fake_files({"app.cfg": "# db\nhost = db\n\nport=5432\n"}) as fake:
        assert read_settings("app.cfg") == {"host": "db", "port": "5432"}
    fake.assert_called_once_with("app.cfg", encoding="utf-8")


def test_value_may_contain_equals_signs() -> None:
    with fake_files({"app.cfg": "url=postgres://h/db?a=b\n"}):
        assert read_settings("app.cfg") == {"url": "postgres://h/db?a=b"}


def test_malformed_line_reports_its_line_number() -> None:
    with (
        fake_files({"app.cfg": "a=1\nbroken\n"}),
        pytest.raises(ValueError, match=r"app\.cfg:2: expected key=value"),
    ):
        read_settings("app.cfg")


def test_blank_key_is_rejected() -> None:
    with fake_files({"app.cfg": " = x\n"}), pytest.raises(ValueError):
        read_settings("app.cfg")


def test_each_path_reads_its_own_text() -> None:
    files = {"a.cfg": "name=a\n", "b.cfg": "name=b\n"}
    with fake_files(files) as fake:
        assert read_settings("b.cfg") == {"name": "b"}
        assert read_settings("a.cfg") == {"name": "a"}
    assert fake.call_count == 2


def test_unknown_path_is_missing() -> None:
    with (
        fake_files({"app.cfg": "a=1\n"}),
        pytest.raises(FileNotFoundError, match="other.cfg"),
    ):
        read_settings("other.cfg")
