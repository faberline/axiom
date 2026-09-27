"""Load a multi-document YAML stream of Kubernetes-style manifests safely."""

from __future__ import annotations

from typing import Any

import yaml


class ManifestError(Exception):
    """Raised when a document in the stream is not a valid manifest."""


def load_manifests(text: str) -> list[dict[str, Any]]:
    """Return every non-empty document, each a mapping with kind and name."""
    manifests: list[dict[str, Any]] = []
    try:
        documents = list(yaml.safe_load_all(text))
    except yaml.YAMLError as exc:
        raise ManifestError(f"invalid YAML: {exc}") from exc
    for index, doc in enumerate(documents):
        if doc is None:
            continue
        if not isinstance(doc, dict):
            raise ManifestError(f"document {index} is not a mapping")
        if not isinstance(doc.get("kind"), str):
            raise ManifestError(f"document {index} has no kind")
        metadata = doc.get("metadata")
        if not metadata.get("name"):
            raise ManifestError(f"document {index} has no metadata.name")
        manifests.append(doc)
    return manifests
