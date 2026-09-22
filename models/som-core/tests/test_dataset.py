"""Loader contract: curation fields pass through unchanged; corpora union safely."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from som_core.dataset import (
    CandidateRecord,
    load_corpora,
    load_family_from_metadata,
    load_python_v2_dataset,
)

REPO_ROOT = Path(__file__).resolve().parents[3]
PYTHON_V2 = REPO_ROOT / "models" / "som-code-python" / "data" / "python-v2"


def write_family(corpus: Path, name: str, *, domain: str = "python", with_curation: bool = True) -> Path:
    fam = corpus / "materials" / name
    fam.mkdir(parents=True)
    candidates = [{"id": "gold", "module": "gold/candidate.py", "kind": "gold"}]
    for i in range(1, 6):
        miss: dict = {
            "id": f"miss_{i}",
            "module": f"miss_{i}/candidate.py",
            "kind": "near_miss",
            "failure_mode": "wrong_branch",
        }
        if with_curation:
            miss["why_wrong"] = f"Branch {i} is taken on the input the fixture sends."
            miss["caught_by"] = ["test_b", "test_a"]
        candidates.append(miss)
    meta = {
        "family_id": name,
        "domain": domain,
        "area": "demo",
        "capability": "demo",
        "requirement": "do the thing",
        "skeleton": "def run(): ...",
        "rationale": {"teaches": "the thing", "why": "the fixture observes it"},
        "oracle": f"fixtures/test_{name}.py" if with_curation else None,
        "candidates": candidates,
    }
    (fam / "family.json").write_text(json.dumps(meta), encoding="utf-8")
    for cand in candidates:
        path = fam / cand["module"]
        path.parent.mkdir()
        path.write_text(f"def run():\n    return {cand['id']!r}\n", encoding="utf-8")
    return fam


def test_curation_fields_pass_through_to_the_training_row(tmp_path: Path) -> None:
    fam = write_family(tmp_path, "00-demo")
    family = load_family_from_metadata(fam)
    gold = family.candidates[0]
    miss = family.candidates[1]
    assert gold.why_wrong is None and gold.caught_by is None
    assert miss.why_wrong == "Branch 1 is taken on the input the fixture sends."
    assert miss.caught_by == ("test_b", "test_a")  # order is the corpus's, never re-sorted here
    row = family.to_decision_dict()
    assert "why_wrong" not in row["candidates"][0]
    assert row["candidates"][1]["why_wrong"] == miss.why_wrong
    assert row["candidates"][1]["caught_by"] == ["test_b", "test_a"]
    assert row["metadata"]["rationale"] == {"teaches": "the thing", "why": "the fixture observes it"}
    assert row["metadata"]["oracle"] == "fixtures/test_00-demo.py"


def test_a_family_without_curation_still_loads(tmp_path: Path) -> None:
    fam = write_family(tmp_path, "01-plain", with_curation=False)
    row = load_family_from_metadata(fam).to_decision_dict()
    assert "why_wrong" not in row["candidates"][1]
    assert "caught_by" not in row["candidates"][1]
    assert row["metadata"]["oracle"] is None


def test_malformed_caught_by_is_refused(tmp_path: Path) -> None:
    fam = write_family(tmp_path, "02-bad")
    meta = json.loads((fam / "family.json").read_text())
    meta["candidates"][1]["caught_by"] = "test_a"
    (fam / "family.json").write_text(json.dumps(meta))
    with pytest.raises(ValueError, match="caught_by must be a list"):
        load_family_from_metadata(fam)


def test_candidate_record_to_dict_emits_caught_by_as_a_list() -> None:
    rec = CandidateRecord(id="miss_1", text="x", kind="near_miss", caught_by=("t",))
    assert rec.to_dict()["caught_by"] == ["t"]
    assert "why_wrong" not in rec.to_dict()


def test_load_corpora_unions_in_the_order_given(tmp_path: Path) -> None:
    a, b = tmp_path / "a", tmp_path / "b"
    write_family(a, "00-alpha")
    write_family(b, "00-beta", domain="rust")
    write_family(b, "01-gamma", domain="rust")
    rows = load_corpora([a, b])
    assert [r["id"] for r in rows] == ["som:python:00-alpha", "som:rust:00-beta", "som:rust:01-gamma"]
    objs = load_corpora([b, a], as_dicts=False)
    assert [f.family_id for f in objs] == ["00-beta", "01-gamma", "00-alpha"]


def test_load_corpora_refuses_a_family_id_two_corpora_claim(tmp_path: Path) -> None:
    a, b = tmp_path / "a", tmp_path / "b"
    write_family(a, "00-same")
    write_family(b, "00-same")
    with pytest.raises(ValueError, match="claimed by both"):
        load_corpora([a, b])


def test_load_corpora_refuses_an_empty_corpus(tmp_path: Path) -> None:
    a, empty = tmp_path / "a", tmp_path / "empty"
    write_family(a, "00-alpha")
    (empty / "materials").mkdir(parents=True)
    with pytest.raises(ValueError, match="yields no family"):
        load_corpora([a, empty])


@pytest.mark.skipif(not PYTHON_V2.is_dir(), reason="python-v2 corpus not checked out")
def test_python_v2_corpus_carries_measured_caught_by_on_every_oracle_family() -> None:
    families = load_python_v2_dataset(PYTHON_V2, as_dicts=False)
    assert len(families) == 101
    with_oracle = [f for f in families if f.metadata.get("oracle") is not None]
    assert len(with_oracle) == 98
    for fam in with_oracle:
        misses = [c for c in fam.candidates if c.kind == "near_miss"]
        assert len(misses) == 5, fam.family_id
        for miss in misses:
            assert miss.why_wrong, (fam.family_id, miss.id)
            assert miss.caught_by, (fam.family_id, miss.id)
        gold = next(c for c in fam.candidates if c.kind == "gold")
        assert gold.caught_by is None and gold.why_wrong is None
    for fam in families:
        if fam.metadata.get("oracle") is None:
            assert all(c.caught_by is None for c in fam.candidates), fam.family_id
