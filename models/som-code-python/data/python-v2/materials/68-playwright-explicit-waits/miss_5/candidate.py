"""Resilient Playwright element waiter preventing UI automation race conditions."""
from typing import Any, Callable, Optional
import time

VALID_STATES = ("visible", "attached", "detached", "hidden")


class ResilientElementWaiter:
    """Explicit wait manager for web automation avoiding fixed sleep race conditions."""

    def __init__(self, default_timeout: float = 10.0, poll_interval: float = 0.05) -> None:
        if default_timeout <= 0:
            raise ValueError("default_timeout must be positive")
        if poll_interval <= 0:
            raise ValueError("poll_interval must be positive")
        self.default_timeout = default_timeout
        self.poll_interval = poll_interval

    def wait_for_element(
        self,
        page: Any,
        selector: str,
        state: str = "visible",
        timeout: Optional[float] = None,
    ) -> Any:
        """Wait explicitly for selector to reach specified state via page.wait_for_selector."""
        if not selector or not isinstance(selector, str):
            raise ValueError("selector must be a non-empty string")
        if state not in VALID_STATES:
            raise ValueError(f"state must be one of {VALID_STATES}, got {state!r}")
        effective_timeout = self.default_timeout if timeout is None else timeout
        if effective_timeout <= 0:
            raise ValueError("timeout must be positive")

        return page.wait_for_selector(selector, state=state, timeout=effective_timeout)

    def wait_and_click(
        self,
        page: Any,
        selector: str,
        timeout: Optional[float] = None,
    ) -> Any:
        """Explicitly wait for element visibility before clicking to prevent race conditions."""
        element = self.wait_for_element(page, selector, state="visible", timeout=timeout)
        element.click()
        return element

    def poll_condition(
        self,
        predicate: Callable[[], bool],
        timeout: Optional[float] = None,
    ) -> bool:
        """Poll predicate until True or timeout, sleeping poll_interval between checks."""
        effective_timeout = self.default_timeout if timeout is None else timeout
        if effective_timeout <= 0:
            raise ValueError("timeout must be positive")
        deadline = time.monotonic() + effective_timeout
        while time.monotonic() < deadline:
            if predicate():
                return True
        return False
