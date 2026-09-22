import types

import pytest


def test_missing_som_internal_runtime_is_not_reported_as_metal(monkeypatch):
    from som.specialists import som_train

    modules = {
        "mlx.core": types.SimpleNamespace(),
        "mlx.nn": types.SimpleNamespace(),
        "mlx.optimizers": types.SimpleNamespace(),
        "mlx.utils": types.SimpleNamespace(tree_map=object()),
    }

    def fake_import(name):
        if name == "som.specialists.som_runtime":
            raise ModuleNotFoundError("missing SOM runtime", name=name)
        return modules[name]

    monkeypatch.setattr(som_train.importlib, "import_module", fake_import)

    with pytest.raises(RuntimeError, match="SOM internal training runtime is unavailable") as error:
        som_train._training_runtime()

    assert "Metal GPU is unavailable" not in str(error.value)


def test_missing_mlx_is_reported_as_metal(monkeypatch):
    from som.specialists import som_train

    def fake_import(name):
        raise ModuleNotFoundError("missing mlx", name=name)

    monkeypatch.setattr(som_train.importlib, "import_module", fake_import)

    with pytest.raises(RuntimeError, match="MLX Metal GPU is unavailable"):
        som_train._training_runtime()


def test_python_smoke_root_uses_repository_corpus_by_default(monkeypatch, tmp_path):
    from som.specialists import som_train

    default = tmp_path / "python-v1"
    monkeypatch.delenv(som_train._PYTHON_SMOKE_CORPUS_ROOT_ENV, raising=False)
    monkeypatch.setattr(som_train, "corpus_path", lambda domain: default)

    assert som_train._python_smoke_corpus_root() == default


@pytest.mark.parametrize("configured, message", [
    ("relative/python-v1", "absolute directory"),
    ("/definitely/not/a/som/python/smoke/root", "existing directory"),
])
def test_python_smoke_root_rejects_invalid_staging_values(monkeypatch, configured, message):
    from som.specialists import som_train

    monkeypatch.setenv(som_train._PYTHON_SMOKE_CORPUS_ROOT_ENV, configured)

    with pytest.raises(ValueError, match=message):
        som_train._python_smoke_corpus_root()


def test_python_smoke_root_rejects_staging_directory_without_smoke(monkeypatch, tmp_path):
    from som.specialists import som_train

    monkeypatch.setenv(som_train._PYTHON_SMOKE_CORPUS_ROOT_ENV, str(tmp_path))

    with pytest.raises(ValueError, match="contain a smoke directory"):
        som_train._python_smoke_corpus_root()


def test_python_smoke_helpers_keep_one_validated_staging_root(monkeypatch, tmp_path):
    from som.specialists import som_train

    staged = tmp_path / "staged-python-v1"
    (staged / "smoke").mkdir(parents=True)
    monkeypatch.setenv(som_train._PYTHON_SMOKE_CORPUS_ROOT_ENV, str(staged))
    calls = []
    audit = {
        "smoke_manifest_sha256": "m" * 64,
        "source_lock_sha256": "s" * 64,
        "family_ledger_sha256": "l" * 64,
        "row_sha256": "r" * 64,
    }
    rows = [{"id": "fixed-row", "missing_correct_patch": False,
             "gold_candidate_id": "gold",
             "candidates": [{"id": "gold", "text": "gold"}, {"id": "miss", "text": "miss"}]}]

    def audited_rows(root):
        calls.append(("rows", root))
        return rows, audit

    def audited(root):
        calls.append(("audit", root))
        return audit

    monkeypatch.setattr(som_train, "audited_python_smoke_rows", audited_rows)
    monkeypatch.setattr(som_train, "audit_python_smoke", audited)
    monkeypatch.setattr(som_train, "verify_v4_start", lambda: {"adapter_sha256": "v" * 64})

    root = som_train._python_smoke_corpus_root()
    assert root == staged.resolve()
    assert som_train._smoke_rows("python", python_corpus_root=root) == rows
    protocol = som_train._smoke_protocol("python", python_corpus_root=root)
    provenance = som_train._smoke_provenance("python", python_corpus_root=root)

    assert protocol["smoke_manifest_sha256"] == audit["smoke_manifest_sha256"]
    assert provenance["row_sha256"] == audit["row_sha256"]
    assert calls == [("rows", root), ("audit", root), ("rows", root), ("audit", root)]
