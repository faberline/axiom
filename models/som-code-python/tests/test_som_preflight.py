import pytest

from som.specialists import som_preflight


def test_preflight_blocks_when_review_or_fixed_oracle_evidence_is_not_ready(monkeypatch):
    monkeypatch.setattr(som_preflight, "verify_som_seed", lambda: {"adapter_sha256": "seed"})
    monkeypatch.setattr(som_preflight, "_control_hashes", lambda: {"rows": {"train-python.jsonl": "row"}})
    monkeypatch.setattr(som_preflight, "_oracle_evidence", lambda: (_ for _ in ()).throw(ValueError("fixed oracle evidence is missing")))
    monkeypatch.setattr(som_preflight.som_data, "_audit", lambda: {"status": "pending_controller_review"})

    result = som_preflight.preflight()

    assert result["status"] == "failed"
    assert result["executes_user_code"] is False
    assert any("fixed oracle evidence" in error for error in result["errors"])
    assert any("named approval" in error for error in result["errors"])
    with pytest.raises(ValueError, match="SOM preflight failed"):
        som_preflight.require_ready()


def test_preflight_accepts_only_frozen_repository_evidence(monkeypatch):
    monkeypatch.setattr(som_preflight, "verify_som_seed", lambda: {"adapter_sha256": "seed"})
    monkeypatch.setattr(som_preflight, "_control_hashes", lambda: {"rows": {"train-python.jsonl": "row"}})
    monkeypatch.setattr(som_preflight, "_oracle_evidence", lambda: {"families": 240, "sha256": "oracle"})
    monkeypatch.setattr(som_preflight.som_data, "_audit", lambda: {"status": "passed"})

    assert som_preflight.require_ready()["status"] == "passed"
