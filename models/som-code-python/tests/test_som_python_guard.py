import pytest


def test_missing_python_smoke_contract_blocks_smoke_before_mlx(monkeypatch, tmp_path):
    from som.specialists import som_data, som_train

    missing = tmp_path / "python-v1"
    monkeypatch.setattr(som_data, "corpus_path", lambda domain: missing)
    monkeypatch.setattr(som_data, "corpus_source_lock", lambda domain: missing / "sources.lock.json")
    monkeypatch.setattr(som_train, "corpus_path", lambda domain: missing)
    monkeypatch.setattr(som_train, "corpus_source_lock", lambda domain: missing / "sources.lock.json")
    monkeypatch.setattr(som_train, "_training_runtime", lambda: (_ for _ in ()).throw(AssertionError("must not import MLX")))

    with pytest.raises(ValueError, match="required Python smoke file is missing"):
        som_train.smoke("python", tmp_path / "run")


def test_python_formal_train_requires_formal_audit_even_with_smoke_result(monkeypatch, tmp_path):
    from som.specialists import som_preflight, som_train

    target = tmp_path / "run"
    target.mkdir()
    (target / "smoke.json").write_text('{"protocol":"som-v1","passed":true}', encoding="utf-8")
    monkeypatch.setattr(som_preflight, "require_ready",
                        lambda domain: (_ for _ in ()).throw(ValueError("formal Python audit is not passed")))
    monkeypatch.setattr(som_train, "_training_runtime",
                        lambda: (_ for _ in ()).throw(AssertionError("must not import MLX")))

    with pytest.raises(ValueError, match="formal Python audit is not passed"):
        som_train.train("python", target)


def test_python_formal_train_requires_a_separate_smoke_result(monkeypatch, tmp_path):
    from som.specialists import som_preflight, som_train

    monkeypatch.setattr(som_preflight, "require_ready", lambda domain: {"status": "passed"})
    monkeypatch.setattr(som_train, "audit_or_raise", lambda domain: {"audit": {"status": "passed"}})
    monkeypatch.setattr(som_train, "_training_runtime",
                        lambda: (_ for _ in ()).throw(AssertionError("must not import MLX")))

    with pytest.raises(ValueError, match="provenance-matching python SOM smoke result"):
        som_train.train("python", tmp_path / "run")


def test_missing_python_corpus_blocks_evaluation_before_model_import(monkeypatch, tmp_path):
    from som.specialists import som_eval, som_preflight

    calls = []

    def blocked(domain):
        calls.append(domain)
        raise ValueError("Python corpus audit is not passed")

    monkeypatch.setattr(som_preflight, "require_ready", blocked)
    monkeypatch.setattr(som_eval, "load_som_model", lambda *_: (_ for _ in ()).throw(AssertionError("must not import model")))

    with pytest.raises(ValueError, match="Python corpus audit is not passed"):
        som_eval.evaluate("python", tmp_path / "run")

    assert calls == ["python"]
