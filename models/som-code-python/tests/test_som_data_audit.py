import pytest

from som.specialists import som_data


def test_rejected_manifest_blocks_audit_before_oracle_or_review(monkeypatch):
    """A rejected corpus cannot become ready through mocked later checks."""
    called = False

    def oracle_that_would_pass():
        nonlocal called
        called = True
        return {"path": "fixed", "sha256": "oracle", "candidates": 1}

    def read_json(path):
        if path == som_data.MANIFEST:
            return {"protocol": "som-v1", "corpus_status": "rejected"}
        if path == som_data.REVIEW_PACKET:
            return {"entries": [{"status": "approved", "approval": "reviewer"}] * 60}
        raise AssertionError(f"unexpected later audit read: {path}")

    monkeypatch.setattr(som_data, "read_json", read_json)
    monkeypatch.setattr(som_data, "executable_oracle_evidence", oracle_that_would_pass)

    with pytest.raises(ValueError, match="not ready for controller review"):
        som_data._audit()

    assert called is False
