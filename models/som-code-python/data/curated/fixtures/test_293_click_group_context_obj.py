import json

from click.testing import CliRunner

from candidate import cli


def run(store, *args, env=None):
    base = ["--store", str(store)] if store else []
    return CliRunner().invoke(cli, [*base, *args], env=env)


def test_add_accumulates_and_persists(tmp_path):
    store = tmp_path / "inv.json"
    assert run(store, "add", "bolt", "3").exit_code == 0
    assert run(store, "add", "bolt", "2").exit_code == 0
    assert run(store, "add", "nut", "1").exit_code == 0
    assert json.loads(store.read_text()) == {"bolt": 5, "nut": 1}
    result = run(store, "list")
    assert result.output == "bolt\t5\nnut\t1\n"


def test_quantity_must_be_positive(tmp_path):
    store = tmp_path / "inv.json"
    result = run(store, "add", "bolt", "0")
    assert result.exit_code == 2
    assert "0" in result.output


def test_store_can_come_from_the_environment(tmp_path):
    store = tmp_path / "env.json"
    result = run(None, "add", "gear", "4", env={"INV_STORE": str(store)})
    assert result.exit_code == 0
    assert json.loads(store.read_text()) == {"gear": 4}


def test_removing_unknown_item_is_a_clean_error(tmp_path):
    store = tmp_path / "inv.json"
    run(store, "add", "bolt", "1")
    result = run(store, "remove", "washer")
    assert result.exit_code == 1
    assert "no such item: washer" in result.output
    assert run(store, "remove", "bolt").exit_code == 0
    assert json.loads(store.read_text()) == {}


def test_verbose_messages_go_to_stderr(tmp_path):
    store = tmp_path / "inv.json"
    result = run(store, "-v", "add", "bolt", "1")
    assert result.stdout == ""
    assert result.stderr == "added 1 bolt\n"
