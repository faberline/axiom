"""Upload a file with metadata as multipart/form-data using requests."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import requests

MAX_UPLOAD_BYTES = 5 * 1024 * 1024
TIMEOUT = 30.0


class UploadTooLargeError(ValueError):
    """Raised before sending a file larger than the upload limit."""

    def __init__(self, size: int, limit: int) -> None:
        super().__init__(f"{size} bytes exceeds the {limit} byte limit")
        self.size = size
        self.limit = limit


def upload(
    session: requests.Session,
    url: str,
    path: Path,
    *,
    description: str,
    content_type: str = "application/octet-stream",
) -> dict[str, Any]:
    """POST ``path`` as the ``file`` part with a ``description`` form field."""
    size = path.stat().st_size
    if size > MAX_UPLOAD_BYTES:
        raise UploadTooLargeError(size, MAX_UPLOAD_BYTES)
    with path.open("rb") as fh:
        resp = session.post(
            url,
            data={"description": description},
            files={"file": (path.name, fh, content_type)},
            timeout=TIMEOUT,
        )
    resp.raise_for_status()
    result: dict[str, Any] = resp.json()
    return result
