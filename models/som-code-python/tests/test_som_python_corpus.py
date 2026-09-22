import json
import hashlib
import runpy
import threading
import time
from pathlib import Path

import pytest

from som.python_corpus import PythonCorpusError, audit_formal_python_corpus
from som.python_smoke import _candidate_count_histograms


def _smoke_builder():
    return runpy.run_path(
        str(Path(__file__).parents[1] / "data/som/python-v1/smoke/build_smoke.py"),
        run_name="som_smoke_builder_test",
    )


def _write(root, relative, value):
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(value, encoding="utf-8")
    return path


def test_formal_audit_rejects_missing_manifest(tmp_path):
    with pytest.raises(PythonCorpusError, match="manifest"):
        audit_formal_python_corpus(tmp_path / "python-v1")


def test_smoke_manifest_cannot_masquerade_as_formal(tmp_path):
    root = tmp_path / "python-v1"
    _write(root, "manifest.json", json.dumps({
        "protocol": "som-v1", "domain": "python", "corpus_kind": "smoke",
        "corpus_status": "ready_for_controller_review", "fixture_root": "proposed-fixtures", "files": {},
    }))
    with pytest.raises(PythonCorpusError, match="smoke fixtures cannot masquerade"):
        audit_formal_python_corpus(root)


def test_smoke_candidate_count_histograms_require_equal_present_and_missing_shapes():
    rows = [
        {"missing_correct_patch": False, "candidates": [{}, {}, {}, {}, {}, {}]},
        {"missing_correct_patch": False, "candidates": [{}, {}, {}, {}, {}, {}]},
        {"missing_correct_patch": True, "candidates": [{}, {}, {}, {}, {}, {}]},
        {"missing_correct_patch": True, "candidates": [{}, {}, {}, {}, {}, {}]},
    ]
    present, missing = _candidate_count_histograms(rows)
    assert present == missing == {6: 2}


def test_smoke_candidate_count_histograms_expose_label_shape_leakage():
    rows = [
        {"missing_correct_patch": False, "candidates": [{}, {}, {}, {}, {}, {}, {}]},
        {"missing_correct_patch": True, "candidates": [{}, {}, {}, {}, {}, {}]},
    ]
    present, missing = _candidate_count_histograms(rows)
    assert present == {7: 1}
    assert missing == {6: 1}


def test_smoke_builder_omits_one_deterministic_near_miss_from_present_rows():
    included_candidates = _smoke_builder()["included_candidates"]
    candidates = [{"id": "gold"}] + [{"id": f"miss_{index}"} for index in range(6)]
    lineage = "python-smoke-deterministic-choice"

    present = included_candidates(candidates, lineage, present=True)
    missing = included_candidates(candidates, lineage, present=False)

    omitted = int(hashlib.sha256(lineage.encode("utf-8")).hexdigest(), 16) % 6
    assert [item["id"] for item in present] == ["gold"] + [
        f"miss_{index}" for index in range(6) if index != omitted
    ]
    assert [item["id"] for item in missing] == [f"miss_{index}" for index in range(6)]
    assert len(present) == len(missing) == 6


def test_smoke_builder_uses_its_own_directory_by_default(monkeypatch):
    builder = _smoke_builder()
    monkeypatch.delenv(builder["OUTPUT_ROOT_ENV"], raising=False)

    assert builder["output_root"]() == builder["ROOT"]


def test_smoke_builder_accepts_an_existing_absolute_staging_directory(tmp_path, monkeypatch):
    builder = _smoke_builder()
    staging_root = tmp_path / "som-python-smoke-stage"
    staging_root.mkdir()
    monkeypatch.setenv(builder["OUTPUT_ROOT_ENV"], str(staging_root))

    assert builder["output_root"]() == staging_root.resolve()


def test_smoke_builder_rejects_staging_root_that_overlaps_source_fixtures(monkeypatch):
    builder = _smoke_builder()
    source_root = builder["SOURCE_ROOT"].resolve()
    monkeypatch.setenv(builder["OUTPUT_ROOT_ENV"], str(source_root))

    with pytest.raises(SystemExit, match="must not overlap source fixtures"):
        builder["output_root"]()

    monkeypatch.setenv(builder["OUTPUT_ROOT_ENV"], str(source_root.parent))
    with pytest.raises(SystemExit, match="must not overlap source fixtures"):
        builder["output_root"]()


def test_smoke_builder_stream_copy_preserves_source_bytes(tmp_path):
    builder = _smoke_builder()
    source_root = tmp_path / "source"
    destination_root = tmp_path / "destination"
    source = _write(source_root, "family-001/candidate.py", "\x00binary\xff\n")

    destination = builder["_copy_fixture_file"](
        source_root, Path("family-001/candidate.py"), destination_root
    )

    assert destination.read_bytes() == source.read_bytes()


def test_smoke_builder_copy_never_executes_fixture_content(tmp_path):
    builder = _smoke_builder()
    source_root = tmp_path / "source"
    destination_root = tmp_path / "destination"
    marker = tmp_path / "fixture-was-executed"
    source = _write(
        source_root,
        "family-001/candidate.py",
        f"from pathlib import Path\nPath({str(marker)!r}).write_text('executed')\n",
    )

    destination = builder["_copy_fixture_file"](
        source_root, source.relative_to(source_root), destination_root
    )

    assert destination.read_bytes() == source.read_bytes()
    assert not marker.exists()


def test_smoke_builder_copy_rejects_source_path_escape(tmp_path):
    builder = _smoke_builder()
    source_root = tmp_path / "source"
    destination_root = tmp_path / "destination"
    _write(tmp_path, "outside.py", "value = 1\n")
    source_root.mkdir()

    with pytest.raises(ValueError, match="simple relative path"):
        builder["_copy_fixture_file"](
            source_root, Path("../outside.py"), destination_root
        )


def test_smoke_builder_copy_timeout_names_the_fixed_fixture_path(tmp_path, monkeypatch):
    builder = _smoke_builder()
    source_root = tmp_path / "source"
    destination_root = tmp_path / "destination"
    _write(source_root, "family-001/candidate.py", "value = 1\n")
    globals_ = builder["_copy_fixture_file"].__globals__

    class TimedOutChild:
        returncode = None

        def __init__(self):
            self.killed = False
            self.calls = 0

        def communicate(self, timeout=None):
            self.calls += 1
            if self.calls == 1:
                raise globals_["subprocess"].TimeoutExpired("fixture-copy", timeout)
            return b"", b""

        def kill(self):
            self.killed = True

    child = TimedOutChild()
    monkeypatch.setattr(globals_["subprocess"], "Popen", lambda *args, **kwargs: child)

    with pytest.raises(SystemExit, match=r"fixture copy timed out after 0.01s: .*candidate.py"):
        builder["_copy_fixture_file"](
            source_root,
            Path("family-001/candidate.py"),
            destination_root,
            timeout_seconds=0.01,
        )
    assert child.killed is True


def test_smoke_builder_stream_copy_rejects_symlink(tmp_path):
    builder = _smoke_builder()
    source_root = tmp_path / "source"
    destination_root = tmp_path / "destination"
    outside = _write(tmp_path, "outside.py", "print('outside')\n")
    link = source_root / "family-001/candidate.py"
    link.parent.mkdir(parents=True)
    try:
        link.symlink_to(outside)
    except OSError as exc:
        pytest.skip(f"symlinks unavailable: {exc}")

    with pytest.raises(ValueError, match="symlink"):
        builder["_copy_fixture_file"](
            source_root, Path("family-001/candidate.py"), destination_root
        )


def test_smoke_hydration_covers_exactly_the_selected_fixed_candidate_files():
    builder = _smoke_builder()
    selected = builder["selected_families"]()
    hydrated = builder["selected_fixture_paths"](selected)
    expected = set()

    for family, family_path, present in selected:
        candidates = builder["included_candidates"](
            family["candidates"], str(family["semantic_lineage"]), present
        )
        for candidate in candidates:
            expected.add(builder["source_relative"](family_path, candidate["module"]))
            expected.add(builder["source_relative"](family_path, candidate["test"]))
            expected.update(
                builder["source_relative"](family_path, support)
                for support in candidate.get("support_paths", [])
            )

    source_root = builder["SOURCE_ROOT"].resolve()
    assert {path.relative_to(source_root) for path in hydrated} == expected
    assert all(path.is_relative_to(source_root) for path in hydrated)


def test_smoke_hydration_timeout_names_the_fixed_fixture_path(tmp_path, monkeypatch):
    builder = _smoke_builder()
    source_root = tmp_path / "source"
    source = _write(source_root, "family-001/candidate.py", "value = 1\n")
    globals_ = builder["_hydrate_fixture_path"].__globals__
    monkeypatch.setitem(globals_, "SOURCE_ROOT", source_root)

    def timeout(*args, **kwargs):
        raise builder["subprocess"].TimeoutExpired(args[0], kwargs["timeout"])

    monkeypatch.setattr(globals_["subprocess"], "run", timeout)

    with pytest.raises(SystemExit, match=r"fixture hydration timed out after 0.01s: .*candidate.py"):
        builder["_hydrate_fixture_path"](source, timeout_seconds=0.01)


def test_smoke_hydration_uses_a_small_bounded_worker_pool(monkeypatch, tmp_path):
    builder = _smoke_builder()
    paths = [tmp_path / f"fixture-{index}.py" for index in range(17)]
    active = 0
    maximum = 0
    lock = threading.Lock()

    def hydrate(path, *, timeout_seconds):
        nonlocal active, maximum
        with lock:
            active += 1
            maximum = max(maximum, active)
        time.sleep(0.01)
        with lock:
            active -= 1

    globals_ = builder["hydrate_selected_fixtures"].__globals__
    monkeypatch.setitem(globals_, "selected_fixture_paths", lambda families: paths)
    monkeypatch.setitem(globals_, "_hydrate_fixture_path", hydrate)

    assert builder["hydrate_selected_fixtures"]([]) == paths
    assert maximum == builder["FIXTURE_HYDRATION_MAX_WORKERS"] == 8


def test_smoke_hydration_rejects_paths_outside_fixed_source_root(tmp_path):
    builder = _smoke_builder()
    source_root = tmp_path / "source"
    source_root.mkdir()

    with pytest.raises(ValueError, match="simple relative path"):
        builder["_fixture_source_path"](source_root, Path("../outside.py"))


def test_formal_audit_does_not_trust_handwritten_passing_certificate(tmp_path):
    root = tmp_path / "python-v1"
    _write(root, "manifest.json", json.dumps({
        "protocol": "som-v1", "domain": "python", "corpus_kind": "formal",
        "corpus_status": "ready_for_controller_review", "fixture_root": "proposed-fixtures", "files": {},
    }))
    _write(root, "reports/family-audit-python.json", json.dumps({"status": "passed", "domain": "python"}))
    with pytest.raises(PythonCorpusError, match="manifest has no hash"):
        audit_formal_python_corpus(root)


def test_fixed_oracle_recomputes_owned_fixture_only(tmp_path):
    from som.python_corpus import recompute_fixed_oracle
    import hashlib

    root = tmp_path / "python-v1"
    source = "def value():\n    return 3\n"
    test = "from candidate import value\n\ndef test_value():\n    assert value() == 3\n"
    source_path = "proposed-fixtures/family-001/gold/candidate.py"
    test_path = "proposed-fixtures/family-001/gold/test_candidate.py"
    _write(root, source_path, source)
    _write(root, test_path, test)
    records = {
        "family-001:gold": {
            "source_path": source_path,
            "expected_exit_status": 0,
            "support_files": [],
            "tests": [{"path": test_path, "sha256": hashlib.sha256(test.encode()).hexdigest()}],
        }
    }
    fresh = recompute_fixed_oracle(root, records)["family-001:gold"]
    assert fresh["observed_pass"] is True
    assert fresh["exit_status"] == 0
    assert fresh["command"][-1] == test_path
    assert "stdout_sha256" not in fresh
    assert "stderr_sha256" not in fresh


def test_formal_audit_forwards_oracle_batch_size(monkeypatch, tmp_path):
    from som import python_corpus
    seen = {}
    monkeypatch.setattr(python_corpus, "_validate_manifest", lambda root: {})
    monkeypatch.setattr(python_corpus, "_source_lock", lambda root, path: {})
    monkeypatch.setattr(python_corpus, "_rows", lambda root: {})
    monkeypatch.setattr(python_corpus, "_validate_rows", lambda root, loaded, lock: {})
    monkeypatch.setattr(python_corpus, "_validate_ledger", lambda root, loaded: [])
    def evidence(root, expected, lock, *, oracle_batch_size):
        seen["size"] = oracle_batch_size
        return {}
    monkeypatch.setattr(python_corpus, "_validate_evidence", evidence)
    monkeypatch.setattr(python_corpus, "_validate_final_approvals", lambda root, ledger: 0)
    monkeypatch.setattr(python_corpus, "sha256", lambda path: "digest")
    python_corpus.audit_formal_python_corpus(tmp_path, oracle_batch_size=13)
    assert seen["size"] == 13


@pytest.mark.parametrize("size", [0, -1, True, False, 1.5])
def test_formal_audit_rejects_invalid_oracle_batch_size(tmp_path, size):
    with pytest.raises(PythonCorpusError, match="batch size"):
        audit_formal_python_corpus(tmp_path, oracle_batch_size=size)


def test_fixed_oracle_batches_at_most_seven_and_keeps_record_order(tmp_path, monkeypatch):
    from som import python_corpus

    root = tmp_path / "python-v1"
    records = {}
    for index in range(8):
        source_path = f"proposed-fixtures/family-{index:03d}/gold/candidate.py"
        test_path = f"proposed-fixtures/family-{index:03d}/gold/test_candidate.py"
        _write(root, source_path, f"def value():\n    return {index}\n")
        test = f"from candidate import value\n\ndef test_value():\n    assert value() == {index}\n"
        _write(root, test_path, test)
        records[f"family-{index:03d}:gold"] = {
            "source_path": source_path,
            "expected_exit_status": 0,
            "support_files": [],
            "tests": [{"path": test_path, "sha256": hashlib.sha256(test.encode()).hexdigest()}],
        }

    batch_sizes = []
    original = python_corpus._run_fixed_oracle_batch

    def observe(root, checked):
        batch_sizes.append(len(checked))
        return original(root, checked)

    monkeypatch.setattr(python_corpus, "_run_fixed_oracle_batch", observe)
    fresh = python_corpus.recompute_fixed_oracle(root, records)

    assert batch_sizes == [7, 1]
    assert list(fresh) == list(records)
    assert all(item["observed_pass"] is True for item in fresh.values())


def test_fixed_oracle_accepts_explicit_larger_batch(tmp_path, monkeypatch):
    from som import python_corpus
    import hashlib

    root = tmp_path / "python-v1"
    records = {}
    for index in range(5):
        source_path = f"proposed-fixtures/family-{index:03d}/gold/candidate.py"
        test_path = f"proposed-fixtures/family-{index:03d}/gold/test_candidate.py"
        _write(root, source_path, f"def value():\n    return {index}\n")
        test = f"from candidate import value\n\ndef test_value():\n    assert value() == {index}\n"
        _write(root, test_path, test)
        records[f"family-{index:03d}:gold"] = {
            "source_path": source_path, "expected_exit_status": 0,
            "support_files": [],
            "tests": [{"path": test_path, "sha256": hashlib.sha256(test.encode()).hexdigest()}],
        }
    batch_sizes = []
    original = python_corpus._run_fixed_oracle_batch
    monkeypatch.setattr(python_corpus, "_run_fixed_oracle_batch", lambda root, checked, **kwargs: (batch_sizes.append(len(checked)) or original(root, checked, **kwargs)))
    fresh = python_corpus.recompute_fixed_oracle(root, records, batch_size=10)
    assert batch_sizes == [5]
    assert list(fresh) == list(records)


def test_fixed_oracle_accepts_relative_root_and_explicit_support_hash(tmp_path):
    from som.python_corpus import recompute_fixed_oracle
    import hashlib
    import os

    root = tmp_path / "python-v1"
    source_path = "proposed-fixtures/family-001/gold/candidate.py"
    test_path = "proposed-fixtures/family-001/gold/test_candidate.py"
    support_path = "proposed-fixtures/family-001/gold/common.py"
    support = "EXPECTED = 3\n"
    _write(root, support_path, support)
    _write(root, source_path, "from common import EXPECTED\ndef value():\n    return EXPECTED\n")
    _write(root, test_path, "from candidate import value\n\ndef test_value():\n    assert value() == 3\n")
    records = {"family-001:gold": {
        "source_path": source_path, "expected_exit_status": 0,
        "support_files": [{"path": support_path, "sha256": hashlib.sha256(support.encode()).hexdigest()}],
        "tests": [{"path": test_path}],
    }}
    previous = os.getcwd()
    try:
        os.chdir(tmp_path)
        fresh = recompute_fixed_oracle(Path("python-v1"), records)["family-001:gold"]
    finally:
        os.chdir(previous)
    assert fresh["support_files"][0]["path"] == support_path


def test_fixed_oracle_rejects_altered_support_hash(tmp_path):
    from som.python_corpus import recompute_fixed_oracle

    root = tmp_path / "python-v1"
    support_path = "proposed-fixtures/family-001/gold/common.py"
    _write(root, support_path, "EXPECTED = 3\n")
    records = {"family-001:gold": {
        "source_path": "proposed-fixtures/family-001/gold/candidate.py",
        "expected_exit_status": 0, "support_files": [{"path": support_path, "sha256": "0" * 64}],
        "tests": [{"path": "proposed-fixtures/family-001/gold/test_candidate.py"}],
    }}
    _write(root, records["family-001:gold"]["source_path"], "def value():\n    return 3\n")
    _write(root, records["family-001:gold"]["tests"][0]["path"], "def test_value():\n    assert True\n")
    with pytest.raises(PythonCorpusError, match="support file bytes changed"):
        recompute_fixed_oracle(root, records)


def test_fixed_oracle_rejects_unsafe_support_file(tmp_path):
    from som.python_corpus import recompute_fixed_oracle

    root = tmp_path / "python-v1"
    source_path = "proposed-fixtures/family-001/gold/candidate.py"
    test_path = "proposed-fixtures/family-001/gold/test_candidate.py"
    support_path = "proposed-fixtures/family-001/gold/common.py"
    _write(root, source_path, "def value():\n    return 3\n")
    _write(root, test_path, "def test_value():\n    assert True\n")
    unsafe = "import subprocess\n"
    _write(root, support_path, unsafe)
    records = {"family-001:gold": {
        "source_path": source_path, "expected_exit_status": 0,
        "support_files": [{"path": support_path, "sha256": hashlib.sha256(unsafe.encode()).hexdigest()}],
        "tests": [{"path": test_path}],
    }}
    with pytest.raises(PythonCorpusError, match="unsafe"):
        recompute_fixed_oracle(root, records)


def test_fixed_oracle_rejects_unsafe_owned_fixture(tmp_path):
    from som.python_corpus import recompute_fixed_oracle

    root = tmp_path / "python-v1"
    source_path = "proposed-fixtures/family-001/gold/candidate.py"
    test_path = "proposed-fixtures/family-001/gold/test_candidate.py"
    _write(root, source_path, "import subprocess\n")
    _write(root, test_path, "def test_value():\n    assert True\n")
    records = {"family-001:gold": {"source_path": source_path, "expected_exit_status": 0,
                                    "support_files": [],
                                    "tests": [{"path": test_path}]}}
    with pytest.raises(PythonCorpusError, match="unsafe"):
        recompute_fixed_oracle(root, records)
