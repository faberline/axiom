"""Poll a condition against a driver the way Selenium's WebDriverWait does."""

from __future__ import annotations

import time
from collections.abc import Callable
from typing import Protocol, TypeVar

T = TypeVar("T")


class WaitTimeoutError(Exception):
    """Raised when the condition never became truthy before the deadline."""


class NoSuchElementError(Exception):
    """Raised by a driver when a locator matches nothing."""


class Element(Protocol):
    """The part of a WebElement the conditions use."""

    def is_displayed(self) -> bool:
        """Return whether the element is visible."""

    @property
    def text(self) -> str:
        """Return the element's visible text."""


class Driver(Protocol):
    """The part of a WebDriver the conditions use."""

    def find_element(self, by: str, value: str) -> Element:
        """Return the first element matching the locator."""


class SystemClock:
    """Monotonic time and real sleeping."""

    def now(self) -> float:
        """Return monotonic seconds."""
        return time.monotonic()

    def sleep(self, seconds: float) -> None:
        """Block for the given number of seconds."""
        time.sleep(seconds)


class Wait:
    """Repeatedly evaluate a condition until it is truthy or time runs out."""

    def __init__(
        self,
        driver: Driver,
        timeout: float,
        poll: float = 0.5,
        clock: SystemClock | None = None,
    ) -> None:
        if timeout < 0 or poll < 0:
            raise ValueError("timeout and poll must be positive")
        self._driver = driver
        self._timeout = timeout
        self._poll = poll
        self._clock = clock or SystemClock()

    def until(self, condition: Callable[[Driver], T], message: str = "") -> T:
        """Return the first truthy condition value, ignoring missing elements."""
        end = self._clock.now() + self._timeout
        while True:
            try:
                value = condition(self._driver)
                if value:
                    return value
            except NoSuchElementError:
                pass
            if self._clock.now() > end:
                raise WaitTimeoutError(message)
            self._clock.sleep(self._poll)

    @property
    def timeout(self) -> float:
        """Return the configured timeout in seconds."""
        return self._timeout


def visibility_of_element_located(
    by: str, value: str
) -> Callable[[Driver], Element | None]:
    """Return the located element once it is displayed, otherwise None."""

    def condition(driver: Driver) -> Element | None:
        element = driver.find_element(by, value)
        return element if element.is_displayed() else None

    return condition


def text_to_be_present(by: str, value: str, text: str) -> Callable[[Driver], bool]:
    """Return whether the located element's text contains the given text."""

    def condition(driver: Driver) -> bool:
        return text in driver.find_element(by, value).text

    return condition
