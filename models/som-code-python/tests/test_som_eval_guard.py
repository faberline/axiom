import pytest


def test_evaluation_stops_at_preflight_before_training_or_mlx(monkeypatch):
    from som.specialists import som_eval, som_preflight

    calls = []

    def blocked():
        calls.append("preflight")
        raise ValueError("SOM preflight failed")

    monkeypatch.setattr(som_preflight, "require_ready", blocked)
    monkeypatch.setattr(som_eval, "verify_pre_final", lambda *_: (_ for _ in ()).throw(AssertionError("must not read data")))
    monkeypatch.setattr(som_eval, "load_som_model", lambda *_: (_ for _ in ()).throw(AssertionError("must not load MLX")))

    with pytest.raises(ValueError, match="SOM preflight failed"):
        som_eval.evaluate("frontend")

    assert calls == ["preflight"]


def test_oracle_provenance_uses_candidate_level_som_evidence(monkeypatch):
    from som.specialists import som_data, som_eval

    monkeypatch.setattr(som_data, "executable_oracle_evidence", lambda: {"path": "fixed", "sha256": "hash", "candidates": 7})

    assert som_eval.oracle_provenance() == {"oracle": "fixed_fixture", "path": "fixed", "sha256": "hash", "candidates": 7}
