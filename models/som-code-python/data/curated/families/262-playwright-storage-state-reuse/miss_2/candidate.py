"""Log in once and reuse a saved Playwright storage_state while it is fresh."""

from __future__ import annotations

import time
from collections.abc import Callable
from pathlib import Path
from typing import Any


def authenticated_context(
    browser: Any,
    state_path: Path,
    login: Callable[[Any], None],
    *,
    max_age: float = 3600.0,
    now: Callable[[], float] = time.time,
) -> Any:
    """Return a logged-in context, reusing state_path when younger than max_age."""
    if now() - state_path.stat().st_mtime < max_age:
        return browser.new_context(storage_state=str(state_path))
    context = browser.new_context()
    try:
        login(context.new_page())
        context.storage_state(path=str(state_path))
    except BaseException:
        context.close()
        raise
    return context
