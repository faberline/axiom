import pytest
import yaml

from candidate import ConfigError, UniqueKeyLoader, load_strict


def test_plain_documents_load():
    assert load_strict("a: 1\nb: [1, 2]\n") == {"a": 1, "b": [1, 2]}


def test_duplicate_top_level_key_reports_its_line():
    with pytest.raises(ConfigError, match="duplicate key 'port'") as info:
        load_strict("host: x\nport: 1\nname: y\nport: 2\n")
    assert info.value.line == 4
    assert str(info.value).startswith("line 4: ")


def test_duplicate_nested_key_is_rejected():
    with pytest.raises(ConfigError, match="duplicate key 'b'") as info:
        load_strict("a:\n  b: 1\n  b: 2\n")
    assert info.value.line == 3


def test_same_key_in_sibling_mappings_is_fine():
    assert load_strict("x: {k: 1}\ny: {k: 2}\n") == {"x": {"k": 1}, "y": {"k": 2}}


def test_merge_keys_may_be_overridden():
    text = "base: &b {a: 1, c: 3}\nderived:\n  <<: *b\n  a: 2\n"
    assert load_strict(text)["derived"] == {"a": 2, "c": 3}


def test_python_tags_are_still_rejected():
    with pytest.raises(ConfigError):
        load_strict("t: !!python/tuple [1, 2]\n")


def test_syntax_error_line_is_one_based():
    with pytest.raises(ConfigError) as info:
        load_strict("a: 1\nb: [2\nc: 3\n")
    assert info.value.line >= 2
    assert info.value.__cause__ is not None


def test_global_safe_loader_is_untouched():
    assert UniqueKeyLoader is not yaml.SafeLoader
    assert yaml.safe_load("a: 1\na: 2\n") == {"a": 2}
