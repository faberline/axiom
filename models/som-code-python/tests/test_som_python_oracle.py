import json
import hashlib
import subprocess
from pathlib import Path

import pytest

from som import python_oracle


def test_fixed_test_checker_accepts_controlled_same_directory_loader(tmp_path):
    path = tmp_path / "test_candidate.py"
    path.write_text("""import importlib.util\nfrom pathlib import Path\n\nmodule_path = Path(__file__).with_name('candidate.py')\nspec = importlib.util.spec_from_file_location('candidate', module_path)\nmodule = importlib.util.module_from_spec(spec)\nspec.loader.exec_module(module)\n""", encoding="utf-8")
    assert python_oracle._unsafe_fixed_test(path) is None


def test_fixed_test_checker_accepts_frozen_fixture_imports(tmp_path):
    path = tmp_path / "test_candidate.py"
    path.write_text(
        "import asyncio, dataclasses, logging, random, sqlite3\n"
        "import fastapi, httpx, pydantic, pytest, sqlalchemy\n"
        "from candidate import solve\n",
        encoding="utf-8",
    )
    assert python_oracle._unsafe_fixed_test(path) is None


def test_fixed_test_checker_accepts_asyncio_and_tmp_path_file(tmp_path):
    path = tmp_path / "test_candidate.py"
    path.write_text(
        "import asyncio\n"
        "def test_io(tmp_path):\n"
        "    marker = tmp_path / 'marker'\n"
        "    marker.write_text('ok')\n"
        "    assert marker.read_text() == 'ok'\n"
        "    asyncio.run(asyncio.sleep(0))\n",
        encoding="utf-8",
    )
    assert python_oracle._unsafe_fixed_test(path) is None


@pytest.mark.parametrize("call", ["asyncio.new_event_loop()", "asyncio.run(asyncio.sleep(0))", "asyncio.sleep(0)", "asyncio.wait_for(asyncio.sleep(0), 1)"])
def test_fixed_test_checker_accepts_frozen_asyncio_calls(tmp_path, call):
    path = tmp_path / "test_candidate.py"
    path.write_text(f"import asyncio\ndef test_async():\n    {call}\n", encoding="utf-8")
    assert python_oracle._unsafe_fixed_test(path) is None


def test_fixed_test_checker_accepts_owned_event_loop_lifecycle(tmp_path):
    path = tmp_path / "test_candidate.py"
    path.write_text("import asyncio\ndef test_async():\n    loop = asyncio.new_event_loop()\n    loop.run_until_complete(asyncio.sleep(0))\n    loop.close()\n", encoding="utf-8")
    assert python_oracle._unsafe_fixed_test(path) is None


def test_fixed_test_checker_accepts_owned_sqlite_and_httpx_resources(tmp_path):
    path = tmp_path / "test_candidate.py"
    path.write_text("import httpx, sqlite3\ndef test_resources():\n    connection = sqlite3.connect(':memory:')\n    client = httpx.Client(transport=httpx.MockTransport(lambda request: None))\n    connection.close()\n    client.close()\n", encoding="utf-8")
    assert python_oracle._unsafe_fixed_test(path) is None


@pytest.mark.parametrize("source", ["import os\n", "import socket\n", "open('escape.txt', 'w')\n"])
def test_fixed_test_checker_rejects_escape_imports_and_calls(tmp_path, source):
    path = tmp_path / "test_candidate.py"
    path.write_text(source, encoding="utf-8")
    assert python_oracle._unsafe_fixed_test(path) is not None


def test_fixed_test_checker_rejects_import_outside_frozen_inventory(tmp_path):
    path = tmp_path / "test_candidate.py"
    path.write_text("import numpy\n", encoding="utf-8")
    assert python_oracle._unsafe_fixed_test(path) is not None


@pytest.mark.parametrize("source", [
    "from pathlib import Path\nPath('x').read_text()\n",
    "marker.write_text('x')\n",
    "import asyncio\nasyncio.create_task(None)\n",
    "import asyncio\ndef test_async():\n    loop = asyncio.get_event_loop()\n    loop.close()\n",
    "import sqlite3\ndef test_db():\n    connection = sqlite3.connect('file.db')\n    connection.close()\n",
    "import httpx\ndef test_http():\n    client = httpx.Client()\n    client.close()\n",
    "def test_close():\n    value = object()\n    value.close()\n",
    "from pathlib import Path\ndef test_io():\n    tmp_path = Path('.')\n    marker = tmp_path / 'marker'\n    marker.write_text('x')\n",
])
def test_fixed_test_checker_rejects_unbound_or_unsafe_file_access(tmp_path, source):
    path = tmp_path / "test_candidate.py"
    path.write_text(source, encoding="utf-8")
    assert python_oracle._unsafe_fixed_test(path) is not None


def _fixture(tmp_path, *, candidates=None, source="def value():\n    return 1\n", test="def test_value():\n    assert True\n"):
    root = tmp_path / "data" / "som" / "python-v1"
    (root / "candidates").mkdir(parents=True)
    (root / "tests").mkdir()
    (root / "candidates" / "candidate.py").write_text(source, encoding="utf-8")
    (root / "tests" / "test_candidate.py").write_text(test, encoding="utf-8")
    if candidates is None:
        source_hash = hashlib.sha256(source.encode()).hexdigest()
        candidates = [{"id": "one", "source": "candidates/candidate.py",
                       "source_sha256": source_hash,
                       "tests": ["tests/test_candidate.py"], "runtime": "pytest",
                       "expected_exit_status": 0, "expected_pass": True}]
    manifest = {"protocol": "som-v1", "fixture_set": "python-v1", "domain": "python",
                "revision": "test-revision", "candidates": candidates}
    (root / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
    return root


def test_python_oracle_emits_deterministic_evidence(tmp_path, monkeypatch):
    root = _fixture(tmp_path)
    monkeypatch.setattr(python_oracle, "OWNED_ROOT", root)
    report = python_oracle.run_python_oracle(fixture_root=root, output=root / "evidence.json")
    assert report["protocol"] == "som-v1"
    assert report["candidates"][0]["observed_pass"] is True
    assert json.loads((root / "evidence.json").read_text())["status"] == "passed"


def test_python_oracle_rejects_missing_metadata(tmp_path, monkeypatch):
    root = tmp_path / "data" / "som" / "python-v1"
    root.mkdir(parents=True)
    monkeypatch.setattr(python_oracle, "OWNED_ROOT", root)
    with pytest.raises(python_oracle.OracleError, match="missing metadata"):
        python_oracle.run_python_oracle(fixture_root=root)


def test_python_oracle_rejects_arbitrary_path(tmp_path):
    with pytest.raises(python_oracle.OracleError, match="outside"):
        python_oracle.run_python_oracle(fixture_root=tmp_path)


def test_python_oracle_rejects_duplicate_ids(tmp_path, monkeypatch):
    candidate = {"id": "same", "source": "candidates/candidate.py",
                 "source_sha256": hashlib.sha256(b"def value():\n    return 1\n").hexdigest(),
                 "tests": ["tests/test_candidate.py"], "runtime": "pytest",
                 "expected_exit_status": 0, "expected_pass": True}
    root = _fixture(tmp_path, candidates=[candidate, dict(candidate)])
    monkeypatch.setattr(python_oracle, "OWNED_ROOT", root)
    with pytest.raises(python_oracle.OracleError, match="duplicate candidate ID"):
        python_oracle.run_python_oracle(fixture_root=root)


def test_python_oracle_rejects_unsafe_dynamic_execution(tmp_path, monkeypatch):
    root = _fixture(tmp_path, source="eval('1')\n")
    monkeypatch.setattr(python_oracle, "OWNED_ROOT", root)
    with pytest.raises(python_oracle.OracleError, match="unsafe dynamic"):
        python_oracle.run_python_oracle(fixture_root=root)


def test_python_oracle_rejects_outcome_mismatch_without_report(tmp_path, monkeypatch):
    candidate = {"id": "one", "source": "candidates/candidate.py",
                 "source_sha256": hashlib.sha256(b"def value():\n    return 1\n").hexdigest(),
                 "tests": ["tests/test_candidate.py"], "runtime": "pytest",
                 "expected_exit_status": 1, "expected_pass": False}
    root = _fixture(tmp_path, candidates=[candidate])
    monkeypatch.setattr(python_oracle, "OWNED_ROOT", root)
    output = root / "evidence.json"
    with pytest.raises(python_oracle.OracleError, match="outcome mismatch"):
        python_oracle.run_python_oracle(fixture_root=root, output=output)
    assert not output.exists()


def test_python_oracle_child_environment_is_fully_isolated(tmp_path, monkeypatch):
    root = _fixture(tmp_path)
    monkeypatch.setattr(python_oracle, "OWNED_ROOT", root)
    seen = {}

    def fake_run(command, **kwargs):
        seen["command"] = command
        seen["env"] = kwargs["env"]
        return subprocess.CompletedProcess(command, 0, "", "")

    monkeypatch.setattr(python_oracle.subprocess, "run", fake_run)
    python_oracle.run_python_oracle(fixture_root=root)
    env = seen["env"]
    assert env["PYTEST_DISABLE_PLUGIN_AUTOLOAD"] == "1"
    assert env["HOME"] != str(tmp_path)
    assert env["TMPDIR"] != str(tmp_path)
    assert env["PYTHONPATH"].endswith("/python-v1")
    assert seen["command"][0].endswith("/python")


@pytest.mark.parametrize("source", [
    "open('outside.txt')\n",
    "from pathlib import Path\nPath('x').read_text()\n",
    "import shutil\nshutil.copy('a', 'b')\n",
    "import glob\nglob.glob('*')\n",
    "import socket\nsocket.socket()\n",
])
def test_python_oracle_rejects_fixture_escape_patterns(tmp_path, monkeypatch, source):
    root = _fixture(tmp_path, source=source)
    monkeypatch.setattr(python_oracle, "OWNED_ROOT", root)
    with pytest.raises(python_oracle.OracleError, match="unsafe"):
        python_oracle.run_python_oracle(fixture_root=root)
