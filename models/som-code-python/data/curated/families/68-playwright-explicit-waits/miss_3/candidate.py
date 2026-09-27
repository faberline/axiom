"""Resilient Playwright element waiter preventing UI automation race conditions."""

import time
from collections.abc import Callable
from typing import Any

VALID_STATES = ("visible", "attached", "detached", "hidden")


class ResilientElementWaiter:
    """Explicit wait manager for web automation avoiding fixed sleep race conditions."""

    def __init__(
        self, default_timeout: float = 10.0, poll_interval: float = 0.05
    ) -> None:
        self.default_timeout = default_timeout
        self.poll_interval = poll_interval

    def wait_for_element(
        self,
        page: Any,
        selector: str,
        state: str = "visible",
        timeout: float | None = None,
    ) -> Any:
        """Wait for selector to reach state through page.wait_for_selector."""
        if not selector or not isinstance(selector, str):
            raise ValueError("selector must be a non-empty string")
        if state not in VALID_STATES:
            raise ValueError(f"state must be one of {VALID_STATES}, got {state!r}")
        effective_timeout = self.default_timeout if timeout is None else timeout

        return page.wait_for_selector(selector, state=state, timeout=effective_timeout)

    def wait_and_click(
        self,
        page: Any,
        selector: str,
        timeout: float | None = None,
    ) -> Any:
        """Wait until the element is visible, then click it."""
        element = self.wait_for_element(
            page, selector, state="visible", timeout=timeout
        )
        element.click()
        return element

    def poll_condition(
        self,
        predicate: Callable[[], bool],
        timeout: float | None = None,
    ) -> bool:
        """Poll predicate until it is true or the timeout passes."""
        effective_timeout = self.default_timeout if timeout is None else timeout
        deadline = time.monotonic() + effective_timeout
        while time.monotonic() < deadline:
            if predicate():
                return True
            time.sleep(self.poll_interval)
        return False
