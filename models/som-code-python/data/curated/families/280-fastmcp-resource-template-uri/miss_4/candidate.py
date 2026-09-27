"""FastMCP-style static resources and URI templates such as users://{id}."""

from __future__ import annotations

import re
from collections.abc import Callable
from urllib.parse import unquote_plus

PARAM_RE = re.compile(r"\{([A-Za-z_][A-Za-z0-9_]*)\}")


class ResourceError(Exception):
    """Raised for malformed templates and URIs that match no resource."""


class ResourceTemplate:
    """Match URIs against a template whose parameters never span a slash."""

    def __init__(self, uri_template: str, fn: Callable[..., str]) -> None:
        parts = PARAM_RE.split(uri_template)
        names = parts[1::2]
        if len(names) != len(set(names)):
            raise ResourceError(f"duplicate parameter in {uri_template!r}")
        pattern = "".join(
            f"(?P<{part}>[^/]+)" if index % 2 else re.escape(part)
            for index, part in enumerate(parts)
        )
        self.uri_template = uri_template
        self.params = tuple(names)
        self._fn = fn
        self._regex = re.compile(pattern)

    def match(self, uri: str) -> dict[str, str] | None:
        """Return percent-decoded parameters when the whole URI matches."""
        found = self._regex.fullmatch(uri)
        if found is None:
            return None
        return {name: unquote_plus(value) for name, value in found.groupdict().items()}

    def read(self, params: dict[str, str]) -> str:
        """Call the resource function with the matched parameters."""
        return self._fn(**params)


class ResourceRegistry:
    """Resolve exact resources first, then templates in registration order."""

    def __init__(self) -> None:
        self._static: dict[str, Callable[[], str]] = {}
        self._templates: list[ResourceTemplate] = []

    def add_resource(self, uri: str, fn: Callable[[], str]) -> None:
        """Register a resource served at exactly this URI."""
        self._static[uri] = fn

    def add_template(self, uri_template: str, fn: Callable[..., str]) -> None:
        """Register a parameterised resource template."""
        self._templates.append(ResourceTemplate(uri_template, fn))

    def read_resource(self, uri: str) -> str:
        """Return the resource content or raise ResourceError."""
        if uri in self._static:
            return self._static[uri]()
        for template in self._templates:
            params = template.match(uri)
            if params is not None:
                return template.read(params)
        raise ResourceError(f"resource not found: {uri}")
