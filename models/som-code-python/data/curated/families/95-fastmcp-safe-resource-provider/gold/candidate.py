"""Serve MCP file resources from one root without path traversal."""

import urllib.parse
from pathlib import Path

from pydantic import BaseModel


class MCPResourceContent(BaseModel):
    """One resource read: its URI, text, MIME type, and size."""

    uri: str
    content: str
    mime_type: str
    size_bytes: int


class SafeMCPResourceProvider:
    """Read-only file resources confined to root_dir and a size limit."""

    def __init__(self, root_dir: str | Path, max_file_size: int = 1_000_000) -> None:
        self.root_dir = Path(root_dir).resolve()
        self.max_file_size = max_file_size

    def is_path_safe(self, rel_path: str) -> bool:
        """Whether rel_path is relative and resolves inside the root."""
        if rel_path.startswith(("/", "\\")) or Path(rel_path).is_absolute():
            return False
        candidate = (self.root_dir / rel_path).resolve()
        return candidate.is_relative_to(self.root_dir)

    def read_resource(self, uri: str) -> MCPResourceContent:
        """Read the file at uri, refusing traversal, directories, and large files."""
        parsed = urllib.parse.urlparse(uri)
        path_component = parsed.netloc + parsed.path if parsed.netloc else parsed.path
        candidate = (self.root_dir / path_component).resolve()

        if (
            path_component.startswith(("/", "\\"))
            or Path(path_component).is_absolute()
            or not candidate.is_relative_to(self.root_dir)
        ):
            raise PermissionError("Access denied: path traversal detected")

        if not candidate.is_file():
            if candidate.is_dir():
                raise IsADirectoryError(
                    "Target path is a directory, not a readable file"
                )
            raise FileNotFoundError(f"Resource not found: {path_component}")

        size = candidate.stat().st_size
        if size > self.max_file_size:
            raise ValueError(
                f"Resource exceeds maximum allowed size ({size} > {self.max_file_size})"
            )

        content = candidate.read_text(encoding="utf-8")
        mime_type = "text/plain"
        if candidate.suffix == ".json":
            mime_type = "application/json"

        return MCPResourceContent(
            uri=uri,
            content=content,
            mime_type=mime_type,
            size_bytes=size,
        )
