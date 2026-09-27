"""Page objects for a login flow over a duck-typed Selenium driver."""

from __future__ import annotations

from typing import Protocol

Locator = tuple[str, str]

USERNAME: Locator = ("id", "username")
PASSWORD: Locator = ("id", "password")
SUBMIT: Locator = ("css selector", "button[type=submit]")
FLASH_ERROR: Locator = ("css selector", ".flash.error")
HEADING: Locator = ("css selector", "h1")
LOGOUT: Locator = ("link text", "Log out")


class LoginError(Exception):
    """Raised when the page is not in the expected state or login fails."""


class Element(Protocol):
    """The part of a WebElement the page objects use."""

    @property
    def text(self) -> str:
        """Return the visible text."""

    def clear(self) -> None:
        """Empty an input."""

    def send_keys(self, value: str) -> None:
        """Type into an input."""

    def click(self) -> None:
        """Click the element."""


class Driver(Protocol):
    """The part of a WebDriver the page objects use."""

    @property
    def current_url(self) -> str:
        """Return the address of the current page."""

    def get(self, url: str) -> None:
        """Navigate to a URL."""

    def find_elements(self, by: str, value: str) -> list[Element]:
        """Return every element matching the locator."""


class Page:
    """Shared navigation and strict element lookup."""

    path = "/"

    def __init__(self, driver: Driver, base_url: str) -> None:
        self._driver = driver
        self._base_url = base_url.rstrip("/")

    @property
    def url(self) -> str:
        """Return the absolute URL of this page."""
        return self._base_url + self.path

    def open(self) -> Page:
        """Navigate to this page and return it."""
        self._driver.get(self.url)
        return self

    def is_current(self) -> bool:
        """Return whether the driver is on this page."""
        return self._driver.current_url.startswith(self.url)

    def _one(self, locator: Locator) -> Element:
        found = self._driver.find_elements(*locator)
        if not found:
            raise LoginError(f"expected one {locator}, found {len(found)}")
        return found[0]


class DashboardPage(Page):
    """The landing page after a successful login."""

    path = "/dashboard"

    def greeting(self) -> str:
        """Return the heading text."""
        return self._one(HEADING).text.strip()

    def logout(self) -> None:
        """Click the logout link."""
        self._one(LOGOUT).click()


class LoginPage(Page):
    """The login form."""

    path = "/login"

    def _type(self, locator: Locator, value: str) -> None:
        field = self._one(locator)
        field.clear()
        field.send_keys(value)

    def login(self, username: str, password: str) -> DashboardPage:
        """Submit credentials and return the dashboard, or raise LoginError."""
        if not username or not password:
            raise LoginError("username and password are required")
        self._type(USERNAME, username)
        self._type(PASSWORD, password)
        self._one(SUBMIT).click()
        errors = self._driver.find_elements(*FLASH_ERROR)
        if errors:
            raise LoginError(errors[0].text.strip())
        dashboard = DashboardPage(self._driver, self._base_url)
        if not dashboard.is_current():
            raise LoginError(f"unexpected page {self._driver.current_url}")
        return dashboard
