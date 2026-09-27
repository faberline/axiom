import pytest
import yaml

from candidate import dump_config, write_config


def test_keys_keep_insertion_order():
    text = dump_config({"zeta": 1, "alpha": 2, "mid": {"b": 1, "a": 2}})
    assert text == "zeta: 1\nalpha: 2\nmid:\n  b: 1\n  a: 2\n"


def test_unicode_is_written_verbatim():
    text = dump_config({"city": "Zürich", "greeting": "你好"})
    assert "Zürich" in text
    assert "你好" in text
    assert "\\u" not in text


def test_lists_use_block_style():
    assert dump_config({"hosts": ["a", "b"]}) == "hosts:\n- a\n- b\n"


def test_arbitrary_objects_are_refused():
    with pytest.raises(yaml.representer.RepresenterError):
        dump_config({"obj": object()})


def test_write_round_trips(tmp_path):
    path = tmp_path / "app.yaml"
    config = {"name": "Café", "port": 8080, "tags": ["x"]}
    write_config(path, config)
    assert yaml.safe_load(path.read_text(encoding="utf-8")) == config


def test_failed_render_leaves_existing_file(tmp_path):
    path = tmp_path / "app.yaml"
    path.write_text("keep: me\n", encoding="utf-8")
    with pytest.raises(yaml.representer.RepresenterError):
        write_config(path, {"bad": object()})
    assert path.read_text(encoding="utf-8") == "keep: me\n"


def test_non_mapping_is_rejected(tmp_path):
    path = tmp_path / "app.yaml"
    with pytest.raises(TypeError, match="config must be a mapping"):
        write_config(path, ["a", "b"])
    assert not path.exists()
