import json

from click.testing import CliRunner

from candidate import main


def run(*args):
    return CliRunner().invoke(main, list(args))


def test_valid_options_are_normalized():
    result = run(
        "--target", "db.local:5432", "--tag", " Prod ", "--tag", "eu-1", "--tag", "prod"
    )
    assert result.exit_code == 0, result.output
    assert json.loads(result.output) == {
        "host": "db.local",
        "port": 5432,
        "tags": ["prod", "eu-1"],
        "timeout": 5.0,
    }


def test_ipv6_style_host_and_port_bounds():
    ok = run("--target", "[::1]:65535", "--timeout", "0.5")
    assert json.loads(ok.output)["host"] == "[::1]"
    assert json.loads(ok.output)["timeout"] == 0.5
    assert run("--target", "h:1").exit_code == 0
    for bad in ["h:0", "h:65536", "h:http", "h", ":80"]:
        result = run("--target", bad)
        assert result.exit_code == 2, bad
        assert "Invalid value for '--target'" in result.output


def test_bad_tags_are_rejected():
    for bad in ["bad!", "9lives", "x" * 21]:
        result = run("--target", "h:1", "--tag", bad)
        assert result.exit_code == 2, bad
        assert "invalid tag" in result.output


def test_timeout_must_be_positive():
    assert run("--target", "h:1", "--timeout", "0").exit_code == 2
    assert run("--target", "h:1", "--timeout", "-1").exit_code == 2


def test_target_is_required():
    result = run()
    assert result.exit_code == 2
    assert "Missing option '--target'" in result.output
