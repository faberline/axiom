import hashlib
import json
from pathlib import Path

import pytest

from som.python_candidate_quality import audit_staged_candidate_quality, render_markdown, write_report


GOLD = """def resolve(value):\n    if value is None:\n        return fallback()\n    return value.strip()\n"""
MISSES = [
    """def resolve(value):\n    if value is None:\n        return fallback()\n    return value\n""",
    """def resolve(value):\n    if not value:\n        return fallback()\n    return value.strip()\n""",
    """def resolve(value):\n    if value is None:\n        return fallback()\n    return value.lower()\n""",
    """def resolve(value):\n    if value is None:\n        return value\n    return value.strip()\n""",
    """def resolve(value):\n    try:\n        return value.strip()\n    except AttributeError:\n        return fallback()\n""",
    """def resolve(value):\n    if value is None:\n        return fallback()\n    return value.rstrip()\n""",
]
MODES = ["wrong_default", "missing_validation", "wrong_boundary", "wrong_branch", "missing_cleanup", "wrong_api_call"]


def make_pack(tmp_path: Path, *, mutate=None):
    family = tmp_path / "family"
    family.mkdir()
    candidates = []
    sources = {"gold": GOLD, **{f"miss_{index + 1}": source for index, source in enumerate(MISSES)}}
    for candidate_id, source in sources.items():
        folder = family / candidate_id
        folder.mkdir()
        (folder / "candidate.py").write_text(source)
        candidates.append({"id": candidate_id, "module": f"{candidate_id}/candidate.py"})
    for index, mode in enumerate(MODES, 1):
        candidates[index]["failure_mode"] = mode
    metadata = {"family_id": "synthetic", "candidates": candidates, "source_hashes": {entry["module"]: hashlib.sha256(sources[entry["id"]].encode()).hexdigest() for entry in candidates}}
    if mutate:
        mutate(metadata, family)
    (family / "family.json").write_text(json.dumps(metadata))
    return tmp_path


def codes(report):
    return {failure["code"] for family in report["families"] for failure in family["failures"]}


def test_accepts_mechanism_level_near_misses_and_writes_reports(tmp_path):
    report = audit_staged_candidate_quality(make_pack(tmp_path))
    assert report["status"] == "passed", report
    json_path, markdown_path = write_report(report, tmp_path / "out")
    assert json_path.is_file()
    assert "Status: **passed**" in markdown_path.read_text()
    assert "candidate quality" in render_markdown(report)


def test_accepts_shared_default_factory_failure_mode(tmp_path):
    def mutate(metadata, _):
        metadata["candidates"][3]["failure_mode"] = "shared_default_factory"

    report = audit_staged_candidate_quality(make_pack(tmp_path, mutate=mutate))
    assert report["status"] == "passed", report


@pytest.mark.parametrize(
    ("mutate", "expected"),
    [
        (lambda meta, _: meta["candidates"].pop(), "near_miss_count"),
        (lambda meta, _: meta["candidates"][1].pop("failure_mode"), "failure_mode_missing"),
        (lambda meta, _: meta["candidates"][2].update(failure_mode="wrong_default"), "failure_mode_duplicate"),
        (lambda meta, _: meta["source_hashes"].update({"miss_1/candidate.py": "bad"}), "source_hash_mismatch"),
        (lambda meta, family: (family / "miss_1" / "candidate.py").write_text("def unrelated():\n    return 7\n"), "ast_overlap_too_low"),
        (lambda meta, family: (family / "miss_1" / "candidate.py").write_text("def resolve(value):\n    return 7\n"), "trivial_literal_return"),
        (lambda meta, family: (family / "miss_1" / "candidate.py").write_text(GOLD + "# only a comment\n"), "comment_or_whitespace_variant"),
    ],
)
def test_rejects_quality_failure_classes(tmp_path, mutate, expected):
    report = audit_staged_candidate_quality(make_pack(tmp_path, mutate=mutate))
    assert report["status"] == "failed"
    assert expected in codes(report)


def test_rejects_active_corpus_root(tmp_path):
    active = tmp_path / "smoke"
    active.mkdir()
    with pytest.raises(ValueError, match="staged"):
        audit_staged_candidate_quality(active)


def test_rejects_duplicate_top_level_entrypoint(tmp_path):
    source = """def solve(value):
    if value is None:
        return fallback()
    return value.strip()

def solve(value):
    return value
"""

    def mutate(meta, family):
        path = family / "miss_1" / "candidate.py"
        path.write_text(source)
        meta["source_hashes"]["miss_1/candidate.py"] = hashlib.sha256(source.encode()).hexdigest()

    report = audit_staged_candidate_quality(make_pack(tmp_path, mutate=mutate))
    assert report["status"] == "failed"
    assert "duplicate_top_level_entrypoint" in codes(report)


def test_rejects_trivial_duplicate_entrypoint_override(tmp_path):
    source = """def solve(value):
    if value is None:
        return fallback()
    return value.strip()

def solve(value):
    return 7
"""

    def mutate(meta, family):
        path = family / "miss_1" / "candidate.py"
        path.write_text(source)
        meta["source_hashes"]["miss_1/candidate.py"] = hashlib.sha256(source.encode()).hexdigest()

    report = audit_staged_candidate_quality(make_pack(tmp_path, mutate=mutate))
    assert report["status"] == "failed"
    assert "duplicate_top_level_entrypoint" in codes(report)
    assert "trivial_entrypoint_override" in codes(report)


def test_rejects_dead_setup_when_gold_returns_setup_derived_value(tmp_path):
    gold = """def solve():
    app = build_app()
    app.configure()
    paths = [route.path for route in app.routes]
    return paths
"""
    dead_setup = """def solve():
    app = build_app()
    app.configure()
    paths = [route.path for route in app.routes]
    return (lambda value: value)(None)
"""

    def mutate(meta, family):
        for candidate_id, source in {"gold": gold, "miss_1": dead_setup}.items():
            module = f"{candidate_id}/candidate.py"
            (family / module).write_text(source)
            meta["source_hashes"][module] = hashlib.sha256(source.encode()).hexdigest()

    report = audit_staged_candidate_quality(make_pack(tmp_path, mutate=mutate))
    assert report["status"] == "failed"
    assert "dead_setup_return" in codes(report)


def test_allows_literal_task_when_gold_does_not_return_setup_value(tmp_path):
    gold = """def solve():
    return True
"""
    candidate = """def solve():
    app = build_app()
    app.configure()
    return (lambda value: value)(None)
"""

    def mutate(meta, family):
        for candidate_id, source in {"gold": gold, "miss_1": candidate}.items():
            module = f"{candidate_id}/candidate.py"
            (family / module).write_text(source)
            meta["source_hashes"][module] = hashlib.sha256(source.encode()).hexdigest()

    report = audit_staged_candidate_quality(make_pack(tmp_path, mutate=mutate))
    assert "dead_setup_return" not in codes(report)
