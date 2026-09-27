import json

from typer.testing import CliRunner

from candidate import app

runner = CliRunner()


def test_table_is_the_default_format():
    result = runner.invoke(app, ["web", "-l", "tier=front", "-l", "app=shop"])
    assert result.exit_code == 0, result.output
    assert result.stdout == "web\t8080\tapp=shop,tier=front\n"


def test_json_format_is_case_insensitive():
    result = runner.invoke(
        app, ["api", "--port", "9000", "--format", "JSON", "-l", "url=a=b"]
    )
    assert result.exit_code == 0, result.output
    assert json.loads(result.stdout) == {
        "name": "api",
        "port": 9000,
        "labels": {"url": "a=b"},
    }


def test_port_range_is_enforced():
    for port in ("0", "65536"):
        result = runner.invoke(app, ["web", "--port", port])
        assert result.exit_code == 2
    assert runner.invoke(app, ["web", "--port", "65535"]).exit_code == 0


def test_malformed_labels_are_usage_errors():
    for bad in ("tier", "=front"):
        result = runner.invoke(app, ["web", "-l", bad])
        assert result.exit_code == 2
        assert "expected KEY=VALUE" in result.stderr


def test_unknown_format_is_rejected():
    assert runner.invoke(app, ["web", "--format", "yaml"]).exit_code == 2
