"""Selenium resilient element accessor with retry recovery on stale DOM references."""
from typing import Any, Optional
import time


class StaleElementReferenceException(Exception):
    """Raised when a reference to an element is no longer attached to the DOM."""
    pass


class ResilientElementAccessor:
    """Accesses and interacts with DOM elements, handling dynamic re-renders and staleness."""

    def __init__(self, max_retries: int = 3, retry_delay: float = 0.05) -> None:
        if max_retries < 1:
            raise ValueError("max_retries must be at least 1")
        if retry_delay < 0:
            raise ValueError("retry_delay cannot be negative")
        self.max_retries = max_retries
        self.retry_delay = retry_delay

    def _is_stale(self, exc: Exception) -> bool:
        """Check if exception is a stale element reference exception."""
        return (
            isinstance(exc, StaleElementReferenceException)
            or exc.__class__.__name__ == "StaleElementReferenceException"
        )

    def get_text(self, driver: Any, by: str, value: str) -> str:
        """Retrieve element text, re-querying driver if element becomes stale."""
        last_error: Optional[Exception] = None
        for attempt in range(self.max_retries):
            try:
                element = driver.find_element(by, value)
                return element.text
            except Exception as exc:
                if self._is_stale(exc):
                    last_error = exc
                    if attempt < self.max_retries - 1 and self.retry_delay > 0:
                        time.sleep(self.retry_delay)
                    continue
                raise
        if last_error is not None:
            raise last_error
        raise StaleElementReferenceException(f"Failed to access element {by}={value}")

    def click(self, driver: Any, by: str, value: str) -> None:
        """Click element, re-querying driver if element becomes stale."""
        last_error: Optional[Exception] = None
        for attempt in range(self.max_retries):
            try:
                element = driver.find_element(by, value)
                element.click()
                return
            except Exception as exc:
                if self._is_stale(exc):
                    last_error = exc
                    if attempt < self.max_retries - 1 and self.retry_delay > 0:
                        time.sleep(self.retry_delay)
                    continue
                raise
        if last_error is not None:
            raise last_error
        raise StaleElementReferenceException(f"Failed to click element {by}={value}")
