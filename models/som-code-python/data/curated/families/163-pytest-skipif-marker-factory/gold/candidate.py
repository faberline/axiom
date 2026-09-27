"""Build skipif markers for tests that need environment variables or tools."""

import os
import shutil

import pytest


def requires_env(*names: str) -> pytest.MarkDecorator:
    """Skip unless every named environment variable is set and non-empty."""
    if not names:
        raise ValueError("name at least one environment variable")
    missing = [name for name in names if not os.environ.get(name)]
    reason = f"missing environment: {', '.join(missing)}"
    return pytest.mark.skipif(bool(missing), reason=reason)


def requires_tool(tool: str) -> pytest.MarkDecorator:
    """Skip unless the executable can be found on PATH."""
    found = shutil.which(tool) is not None
    return pytest.mark.skipif(not found, reason=f"{tool} not found on PATH")
