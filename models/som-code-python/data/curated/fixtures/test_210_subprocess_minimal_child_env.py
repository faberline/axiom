import json
import subprocess
import sys
from pathlib import Path

import pytest

from candidate import child_env, run_isolated

DUMP = (
    "import json, os; print(json.dumps({'cwd': os.getcwd(), 'env': dict(os.environ)}))"
)


def test_only_allowlisted_variables_are_inherited():
    base = {"PATH": "/bin", "HOME": "/home/me", "AWS_SECRET": "x", "PS1": "$"}
    assert child_env(base=base) == {"PATH": "/bin", "HOME": "/home/me"}


def test_overrides_are_added_and_win():
    env = child_env({"PATH": "/opt/bin", "MODE": "test"}, base={"PATH": "/bin"})
    assert env == {"PATH": "/opt/bin", "MODE": "test"}


def test_invalid_override_names_raise():
    for bad in ({"": "x"}, {"A=B": "x"}):
        with pytest.raises(ValueError, match="invalid variable name"):
            child_env(bad, base={})


def test_child_sees_cwd_and_no_secrets(tmp_path, monkeypatch):
    monkeypatch.setenv("SECRET_TOKEN", "hunter2")
    result = run_isolated(
        [sys.executable, "-c", DUMP], cwd=tmp_path, overrides={"MODE": "ci"}
    )
    seen = json.loads(result.stdout)
    assert Path(seen["cwd"]).resolve() == tmp_path.resolve()
    assert "SECRET_TOKEN" not in seen["env"]
    assert seen["env"]["MODE"] == "ci"


def test_bad_cwd_and_failures_raise(tmp_path):
    with pytest.raises(NotADirectoryError):
        run_isolated([sys.executable, "-c", "pass"], cwd=tmp_path / "missing")
    with pytest.raises(subprocess.CalledProcessError) as info:
        run_isolated([sys.executable, "-c", "raise SystemExit(4)"], cwd=tmp_path)
    assert info.value.returncode == 4
