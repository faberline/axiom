import pytest

from candidate import ManifestError, load_manifests

STREAM = """\
kind: Service
metadata:
  name: web
---
---
kind: Deployment
metadata:
  name: web
spec:
  replicas: 2
...
"""


def test_every_document_is_returned_in_order():
    docs = load_manifests(STREAM)
    assert [d["kind"] for d in docs] == ["Service", "Deployment"]
    assert docs[1]["spec"] == {"replicas": 2}


def test_empty_stream_is_an_empty_list():
    assert load_manifests("") == []
    assert load_manifests("---\n---\n") == []


def test_python_tags_are_rejected():
    text = "kind: !!python/object/apply:os.getcwd []\nmetadata: {name: x}\n"
    with pytest.raises(ManifestError, match="invalid YAML"):
        load_manifests(text)


def test_syntax_errors_are_wrapped():
    with pytest.raises(ManifestError, match="invalid YAML") as info:
        load_manifests("kind: [unclosed\n")
    assert info.value.__cause__ is not None


def test_non_mapping_document_reports_its_index():
    text = "kind: A\nmetadata: {name: a}\n---\n- just\n- a list\n"
    with pytest.raises(ManifestError, match="document 1 is not a mapping"):
        load_manifests(text)


def test_kind_must_be_a_string():
    with pytest.raises(ManifestError, match="document 0 has no kind"):
        load_manifests("metadata: {name: a}\n")
    with pytest.raises(ManifestError, match="document 0 has no kind"):
        load_manifests("kind: 3\nmetadata: {name: a}\n")


def test_metadata_name_is_required():
    with pytest.raises(ManifestError, match="has no metadata.name"):
        load_manifests("kind: A\n")
    with pytest.raises(ManifestError, match="has no metadata.name"):
        load_manifests("kind: A\nmetadata: {name: ''}\n")
    with pytest.raises(ManifestError, match="has no metadata.name"):
        load_manifests("kind: A\nmetadata: [name]\n")
