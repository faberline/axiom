"""Fail-closed execution harness for the future ``python-v1`` oracle.

The harness is deliberately separate from SOM inference.  It runs only
repository-owned fixture files and records evidence only after every declared
outcome matches.  A request is never passed to this module.
"""

from __future__ import annotations

import ast
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from typing import Any


PROTOCOL = "som-v1"
FIXTURE_NAME = "python-v1"
REPO_ROOT = Path(__file__).resolve().parents[1]
OWNED_ROOT = REPO_ROOT / "data" / "som" / FIXTURE_NAME
MANIFEST_NAME = "manifest.json"
REPORT_NAME = "evidence.json"
_TIMEOUT_SECONDS = 30


class OracleError(ValueError):
    """A fixture is incomplete, unsafe, or did not meet its frozen oracle."""


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def _inside(path: Path, root: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
    except ValueError:
        return False
    return True


def _relative_file(root: Path, value: Any, label: str) -> Path:
    if not isinstance(value, str) or not value or Path(value).is_absolute():
        raise OracleError(f"{label} must be a relative fixture path")
    path = (root / value).resolve()
    if not _inside(path, root) or not path.is_file():
        raise OracleError(f"{label} is outside the owned fixture tree or missing: {value}")
    return path


def _unsafe_python(path: Path) -> str | None:
    try:
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    except (OSError, SyntaxError) as exc:
        return f"cannot parse {path.name}: {exc}"
    blocked_imports = {
        "builtins", "code", "codeop", "ctypes", "glob", "importlib", "marshal",
        "os", "pathlib", "pty", "requests", "selectors", "shutil", "shlex",
        "socket", "subprocess", "sys", "telnetlib", "urllib", "webbrowser",
    }
    blocked_calls = {"__import__", "compile", "eval", "exec", "breakpoint"}
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            names = [alias.name.split(".", 1)[0] for alias in node.names]
            if any(name in blocked_imports for name in names):
                return f"unsafe dynamic or external import in {path.name}"
        elif isinstance(node, ast.ImportFrom):
            name = (node.module or "").split(".", 1)[0]
            if name in blocked_imports:
                return f"unsafe dynamic or external import in {path.name}"
        elif isinstance(node, ast.Call):
            name = node.func.id if isinstance(node.func, ast.Name) else None
            attr = node.func.attr if isinstance(node.func, ast.Attribute) else None
            if name in blocked_calls or name == "open" or attr in {"run", "Popen", "system", "popen", "serve",
                                                 "open", "read_text", "read_bytes", "write_text", "write_bytes",
                                                 "read_text", "read_bytes", "write_text", "write_bytes",
                                                 "create_subprocess_exec", "create_subprocess_shell",
                                                 "start_server", "open_connection"}:
                return f"unsafe dynamic execution in {path.name}"
    return None


def _unsafe_fixed_test(path: Path) -> str | None:
    """Check a repository-owned test using the controlled candidate loader.

    Fixed tests may import the frozen fixture modules and use the controlled
    same-directory candidate loader.  They may also call ``asyncio.run`` and
    read or write a path derived from pytest's injected ``tmp_path`` fixture.
    All other process, network, and filesystem access remains forbidden.
    """
    try:
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    except (OSError, SyntaxError) as exc:
        return f"cannot parse {path.name}: {exc}"
    allowed_imports = {
        "asyncio", "candidate", "dataclasses", "fastapi", "httpx", "importlib",
        "logging", "pathlib", "pydantic", "pytest", "random", "sqlalchemy", "sqlite3",
    }
    blocked_calls = {"__import__", "compile", "eval", "exec", "breakpoint", "open"}
    blocked_attrs = {"run", "Popen", "system", "popen", "serve", "open", "read_text", "read_bytes",
                     "write_text", "write_bytes", "create_subprocess_exec", "create_subprocess_shell",
                     "start_server", "open_connection"}
    allowed_tmp_path_names: set[str] = set()
    event_loop_names: set[str] = set()
    resource_names: set[str] = set()

    def tmp_path_expression(node: ast.AST, parameter: str) -> bool:
        if isinstance(node, ast.Name):
            return node.id == parameter
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Div):
            return tmp_path_expression(node.left, parameter)
        return False

    class _TmpPathAssignments(ast.NodeVisitor):
        def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
            self._visit_function(node)

        def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
            self._visit_function(node)

        def _visit_function(self, node: ast.FunctionDef | ast.AsyncFunctionDef) -> None:
            parameters = {arg.arg for arg in (*node.args.posonlyargs, *node.args.args, *node.args.kwonlyargs) if arg.arg == "tmp_path"}
            for child in ast.walk(node):
                if parameters and isinstance(child, ast.Assign) and tmp_path_expression(child.value, next(iter(parameters))):
                    for target in child.targets:
                        if isinstance(target, ast.Name):
                            allowed_tmp_path_names.add(target.id)
                if isinstance(child, ast.Assign) and isinstance(child.value, ast.Call):
                    call = child.value.func
                    if (isinstance(call, ast.Attribute) and isinstance(call.value, ast.Name)
                            and call.value.id == "asyncio" and call.attr == "new_event_loop"):
                        for target in child.targets:
                            if isinstance(target, ast.Name):
                                event_loop_names.add(target.id)
                    sqlite_memory = (
                        isinstance(call, ast.Attribute) and isinstance(call.value, ast.Name)
                        and call.value.id == "sqlite3" and call.attr == "connect"
                        and len(child.value.args) == 1 and isinstance(child.value.args[0], ast.Constant)
                        and child.value.args[0].value == ":memory:"
                    )
                    httpx_mock = (
                        isinstance(call, ast.Attribute) and isinstance(call.value, ast.Name)
                        and call.value.id == "httpx" and call.attr == "Client"
                        and any(keyword.arg == "transport" and isinstance(keyword.value, ast.Call)
                                and isinstance(keyword.value.func, ast.Attribute)
                                and isinstance(keyword.value.func.value, ast.Name)
                                and keyword.value.func.value.id == "httpx"
                                and keyword.value.func.attr == "MockTransport"
                                for keyword in child.value.keywords)
                    )
                    if sqlite_memory or httpx_mock:
                        for target in child.targets:
                            if isinstance(target, ast.Name):
                                resource_names.add(target.id)

    _TmpPathAssignments().visit(tree)
    def is_allowed_tmp_name(node: ast.AST) -> bool:
        return isinstance(node, ast.Name) and node.id in allowed_tmp_path_names
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name.split(".", 1)[0] not in allowed_imports:
                    return f"unsafe fixed-test import in {path.name}"
        elif isinstance(node, ast.ImportFrom):
            if (node.module or "").split(".", 1)[0] not in allowed_imports:
                return f"unsafe fixed-test import in {path.name}"
        elif isinstance(node, ast.Call):
            name = node.func.id if isinstance(node.func, ast.Name) else None
            attr = node.func.attr if isinstance(node.func, ast.Attribute) else None
            if isinstance(node.func, ast.Attribute) and isinstance(node.func.value, ast.Name) and node.func.value.id == "asyncio":
                if attr in {"new_event_loop", "run", "sleep", "wait_for"}:
                    continue
                return f"unsafe fixed-test execution in {path.name}"
            if attr in {"run_until_complete", "close"} and isinstance(node.func.value, ast.Name):
                if node.func.value.id in event_loop_names or node.func.value.id in resource_names:
                    continue
                return f"unsafe fixed-test execution in {path.name}"
            if attr in {"read_text", "write_text"} and isinstance(node.func.value, ast.Name):
                if is_allowed_tmp_name(node.func.value):
                    continue
            if name in blocked_calls or attr in blocked_attrs:
                return f"unsafe fixed-test execution in {path.name}"
    return None


def _load_manifest(root: Path) -> dict[str, Any]:
    manifest_path = root / MANIFEST_NAME
    if not manifest_path.is_file():
        raise OracleError(f"missing metadata: {manifest_path}")
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise OracleError(f"invalid metadata: {exc}") from exc
    if not isinstance(manifest, dict):
        raise OracleError("metadata must be an object")
    if manifest.get("protocol") != PROTOCOL or manifest.get("domain") != "python":
        raise OracleError("metadata must declare protocol som-v1 and domain python")
    if manifest.get("fixture_set") != FIXTURE_NAME:
        raise OracleError("metadata fixture_set must be python-v1")
    if not isinstance(manifest.get("revision"), str) or not manifest["revision"].strip():
        raise OracleError("missing metadata: revision")
    candidates = manifest.get("candidates")
    if not isinstance(candidates, list) or not candidates:
        raise OracleError("missing metadata: candidates")
    ids: set[str] = set()
    for candidate in candidates:
        if not isinstance(candidate, dict):
            raise OracleError("candidate metadata must be an object")
        ident = candidate.get("id")
        if not isinstance(ident, str) or not ident.strip():
            raise OracleError("missing candidate id")
        if ident in ids:
            raise OracleError(f"duplicate candidate ID: {ident}")
        ids.add(ident)
        if not isinstance(candidate.get("source"), str):
            raise OracleError(f"missing source metadata for candidate {ident}")
        if not isinstance(candidate.get("source_sha256"), str) or len(candidate["source_sha256"]) != 64:
            raise OracleError(f"missing source_sha256 metadata for candidate {ident}")
        if not isinstance(candidate.get("tests"), list) or not candidate["tests"]:
            raise OracleError(f"missing tests metadata for candidate {ident}")
        if not isinstance(candidate.get("expected_exit_status"), int):
            raise OracleError(f"missing expected_exit_status for candidate {ident}")
        if not isinstance(candidate.get("expected_pass"), bool):
            raise OracleError(f"missing expected_pass for candidate {ident}")
        if candidate.get("runtime") not in {"pytest", "sqlite"}:
            raise OracleError(f"candidate {ident} must declare pytest or sqlite runtime")
    return manifest


def _validate_files(root: Path, manifest: dict[str, Any]) -> list[dict[str, Any]]:
    checked = []
    for candidate in manifest["candidates"]:
        ident = candidate["id"]
        source = _relative_file(root, candidate["source"], f"candidate {ident} source")
        paths = [source]
        for test in candidate["tests"]:
            paths.append(_relative_file(root, test, f"candidate {ident} test"))
        for path in paths:
            if path.suffix != ".py":
                raise OracleError(f"fixture code must be Python: {path.name}")
            reason = _unsafe_python(path)
            if reason:
                raise OracleError(reason)
        if candidate["source_sha256"] != _sha256(source):
            raise OracleError(f"candidate {ident} source hash mismatch")
        checked.append({"metadata": candidate, "paths": paths, "source": source})
    return checked


def run_python_oracle(*, fixture_root: Path = OWNED_ROOT, output: Path | None = None) -> dict[str, Any]:
    """Run a pinned fixture set and return deterministic evidence.

    ``fixture_root`` is accepted for testability but must resolve to the
    repository-owned ``data/som/python-v1`` directory.  The report is written
    only after all expected outcomes match.
    """
    root = Path(fixture_root).resolve()
    if root != OWNED_ROOT.resolve():
        raise OracleError("fixture root is outside the owned python-v1 tree")
    manifest = _load_manifest(root)
    checked = _validate_files(root, manifest)
    observations = []
    with tempfile.TemporaryDirectory(prefix="som-v1-python-oracle-") as temp_name:
        temp = Path(temp_name)
        shutil.copytree(root, temp / FIXTURE_NAME, dirs_exist_ok=True)
        staged = temp / FIXTURE_NAME
        for item in checked:
            candidate = item["metadata"]
            tests = [str(Path(test)) for test in candidate["tests"]]
            command = [sys.executable, "-m", "pytest", "-q", "--disable-warnings", *tests]
            isolated_home = temp / "home"
            isolated_tmp = temp / "tmp"
            isolated_home.mkdir(exist_ok=True)
            isolated_tmp.mkdir(exist_ok=True)
            env = {
                "PATH": os.environ.get("PATH", ""),
                "HOME": str(isolated_home),
                "TMPDIR": str(isolated_tmp),
                "PYTHONPATH": str(staged),
                "PYTHONHASHSEED": "0",
                "PYTHONDONTWRITEBYTECODE": "1",
                "PYTEST_DISABLE_PLUGIN_AUTOLOAD": "1",
            }
            if candidate["runtime"] == "sqlite":
                env["SOM_SQLITE_ROOT"] = str(temp / "sqlite")
                (temp / "sqlite").mkdir()
            proc = subprocess.run(command, cwd=staged, env=env, text=True,
                                  capture_output=True, timeout=_TIMEOUT_SECONDS, check=False)
            observed_pass = proc.returncode == 0
            expected_pass = candidate["expected_pass"]
            expected_status = candidate["expected_exit_status"]
            if observed_pass != expected_pass or proc.returncode != expected_status:
                raise OracleError(
                    f"candidate {candidate['id']} outcome mismatch: "
                    f"expected pass={expected_pass}, exit={expected_status}; "
                    f"observed pass={observed_pass}, exit={proc.returncode}"
                )
            observations.append({
                "id": candidate["id"],
                "runtime": candidate["runtime"],
                "source": candidate["source"],
                "source_sha256": _sha256(staged / candidate["source"]),
                "tests": tests,
                "command": command,
                "exit_status": proc.returncode,
                "observed_pass": observed_pass,
                "stdout_sha256": hashlib.sha256(proc.stdout.encode()).hexdigest(),
                "stderr_sha256": hashlib.sha256(proc.stderr.encode()).hexdigest(),
            })
    report = {"protocol": PROTOCOL, "name": FIXTURE_NAME, "revision": manifest["revision"],
              "status": "passed", "candidates": observations}
    if output is not None:
        output = Path(output)
        if not _inside(output.resolve(), OWNED_ROOT):
            raise OracleError("evidence output must be inside the owned fixture tree")
        output.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    return report
