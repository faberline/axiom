"""Loader contract: curation fields pass through unchanged; corpora union safely."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from som_core.dataset import (
    CandidateRecord,
    load_corpora,
    find_default_corpora,
    load_corpus,
    load_family_from_metadata,
    parse_layer_weights,
    split_dataset,
    weighted_epoch,
)

REPO_ROOT = Path(__file__).resolve().parents[3]
CURATED = REPO_ROOT / "models" / "som-code-python" / "data" / "curated"


def write_family(
    corpus: Path,
    name: str,
    *,
    domain: str = "python",
    with_curation: bool = True,
    overrides: str | None = None,
) -> Path:
    fam = corpus / "families" / name
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
        "caption": "A demo module whose run function returns its own candidate id.",
        "rationale": {"teaches": "the thing", "why": "the fixture observes it"},
        "oracle": f"fixtures/test_{name}.py" if with_curation else None,
        "decompiled": {"surface": [{"kind": "function", "name": "run", "params": []}], "imports": [], "raises": [], "status_codes": []},
        "candidates": candidates,
    }
    if overrides is not None:
        meta["overrides"] = overrides
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
    row = family.to_row()
    assert "why_wrong" not in row["candidates"][0]
    assert row["candidates"][1]["why_wrong"] == miss.why_wrong
    assert row["candidates"][1]["caught_by"] == ["test_b", "test_a"]
    assert row["metadata"]["rationale"] == {"teaches": "the thing", "why": "the fixture observes it"}
    assert row["metadata"]["oracle"] == "fixtures/test_00-demo.py"
    assert row["metadata"]["caption"] == "A demo module whose run function returns its own candidate id."
    assert row["metadata"]["decompiled"]["surface"][0]["name"] == "run"


def test_a_family_without_curation_still_loads(tmp_path: Path) -> None:
    fam = write_family(tmp_path, "01-plain", with_curation=False)
    row = load_family_from_metadata(fam).to_row()
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
    (empty / "families").mkdir(parents=True)
    with pytest.raises(ValueError, match="yields no family"):
        load_corpora([a, empty])


def test_default_corpora_add_the_user_layer_only_when_it_has_a_family(tmp_path: Path) -> None:
    data = tmp_path / "models" / "som-code-python" / "data"
    write_family(data / "curated", "00-ours")
    (data / "user" / "families").mkdir(parents=True)
    assert find_default_corpora(tmp_path) == [(data / "curated").resolve()]
    write_family(data / "user", "90-mine")
    dirs = find_default_corpora(tmp_path)
    assert dirs == [(data / "curated").resolve(), (data / "user").resolve()]
    assert [r["id"] for r in load_corpora(dirs)] == ["som:python:00-ours", "som:python:90-mine"]


def test_a_user_family_cannot_shadow_a_curated_one(tmp_path: Path) -> None:
    data = tmp_path / "models" / "som-code-python" / "data"
    write_family(data / "curated", "00-same")
    write_family(data / "user", "00-same")
    with pytest.raises(ValueError, match="claimed by both"):
        load_corpora(find_default_corpora(tmp_path))


def test_every_row_names_its_layer(tmp_path: Path) -> None:
    data = tmp_path / "data"
    write_family(data / "curated", "00-ours")
    write_family(data / "user", "90-mine")
    rows = load_corpora([data / "curated", data / "user"])
    assert {r["id"]: r["layer"] for r in rows} == {
        "som:python:00-ours": "curated",
        "som:python:90-mine": "user",
    }


def test_a_user_family_that_declares_overrides_replaces_the_curated_one(tmp_path: Path) -> None:
    data = tmp_path / "data"
    write_family(data / "curated", "00-ours")
    write_family(data / "curated", "01-kept")
    write_family(data / "user", "90-team", overrides="som:python:00-ours")
    rows = load_corpora([data / "curated", data / "user"])
    assert [r["id"] for r in rows] == ["som:python:01-kept", "som:python:90-team"]
    assert rows[1]["metadata"]["overrides"] == "som:python:00-ours"
    objs = load_corpora([data / "curated", data / "user"], as_dicts=False)
    assert [f.family_id for f in objs] == ["01-kept", "90-team"]


def test_an_override_of_an_id_nobody_loads_is_refused(tmp_path: Path) -> None:
    data = tmp_path / "data"
    write_family(data / "curated", "00-ours")
    write_family(data / "user", "90-team", overrides="som:python:00-typo")
    with pytest.raises(ValueError, match="which no earlier corpus loads"):
        load_corpora([data / "curated", data / "user"])


def test_the_curated_layer_cannot_override(tmp_path: Path) -> None:
    a, b = tmp_path / "a", tmp_path / "curated"
    write_family(a, "00-ours")
    write_family(b, "01-other", overrides="som:python:00-ours")
    with pytest.raises(ValueError, match="only the user layer may override"):
        load_corpora([a, b])


def test_one_target_takes_one_override(tmp_path: Path) -> None:
    data = tmp_path / "data"
    write_family(data / "curated", "00-ours")
    write_family(data / "user", "90-team", overrides="som:python:00-ours")
    write_family(data / "user", "91-team", overrides="som:python:00-ours")
    with pytest.raises(ValueError, match="overridden by both"):
        load_corpora([data / "curated", data / "user"])


def rows_of(layer: str, n: int) -> list[dict]:
    return [{"id": f"{layer}-{i}", "layer": layer} for i in range(n)]


def test_weighted_epoch_repeats_each_row_by_its_layer_weight() -> None:
    rows = rows_of("curated", 3) + rows_of("user", 3)
    out = weighted_epoch(rows, {"curated": 1.0, "user": 2.0})
    counts = {r["id"]: sum(1 for o in out if o is r) for r in rows}
    assert counts == {**{f"curated-{i}": 1 for i in range(3)}, **{f"user-{i}": 2 for i in range(3)}}
    only_user = weighted_epoch(rows, {"curated": 0.0, "user": 1.0})
    assert sorted(r["id"] for r in only_user) == [f"user-{i}" for i in range(3)]


def test_a_fractional_weight_is_its_expected_count_and_reproducible() -> None:
    rows = rows_of("user", 400)
    out = weighted_epoch(rows, {"curated": 1.0, "user": 1.5}, seed=7)
    counts = [sum(1 for o in out if o is r) for r in rows]
    assert set(counts) == {1, 2}
    assert 540 <= len(out) <= 660
    assert [r["id"] for r in out] == [r["id"] for r in weighted_epoch(rows, {"curated": 1.0, "user": 1.5}, seed=7)]


def test_default_weights_train_user_rows_twice() -> None:
    assert parse_layer_weights(None) == {"curated": 1.0, "user": 2.0}
    assert parse_layer_weights(["user=3"]) == {"curated": 1.0, "user": 3.0}
    assert parse_layer_weights("user=0.5, curated=0") == {"curated": 0.0, "user": 0.5}


@pytest.mark.parametrize("spec, message", [
    ("team=2", "must be one of"),
    ("user", "must be one of"),
    ("user=lots", "is not a number"),
    ("user=-1", "must be zero or more"),
    ("user=nan", "must be zero or more"),
    ("user=inf", "must be zero or more"),
])
def test_a_malformed_layer_weight_is_refused(spec: str, message: str) -> None:
    with pytest.raises(ValueError, match=message):
        parse_layer_weights([spec])


def test_split_keeps_validation_rows_for_every_layer() -> None:
    rows = rows_of("curated", 12) + rows_of("user", 4)
    train_rows, val_rows = split_dataset(rows)
    assert {r["layer"] for r in val_rows} == {"curated", "user"}
    assert not {r["id"] for r in train_rows} & {r["id"] for r in val_rows}
    lone_train, lone_val = split_dataset(rows_of("curated", 12) + rows_of("user", 1))
    assert "user-0" in {r["id"] for r in lone_train}
    assert "user-0" not in {r["id"] for r in lone_val}
    single = rows_of("curated", 12)
    assert split_dataset(single) == split_dataset(single, stratify_by=None)


@pytest.mark.skipif(not CURATED.is_dir(), reason="curated corpus not checked out")
def test_curated_corpus_carries_measured_caught_by_on_every_oracle_family() -> None:
    families = load_corpus(CURATED, as_dicts=False)
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
        assert isinstance(fam.metadata.get("caption"), str), fam.family_id
        assert isinstance(fam.metadata.get("decompiled", {}).get("surface"), list), fam.family_id
        if fam.metadata.get("oracle") is None:
            assert all(c.caught_by is None for c in fam.candidates), fam.family_id
