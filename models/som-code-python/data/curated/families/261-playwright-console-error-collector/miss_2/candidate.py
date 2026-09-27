"""Fail a Playwright test on browser console errors and uncaught page errors."""

from __future__ import annotations

from collections.abc import Iterable
from types import TracebackType
from typing import Any


class ConsoleErrorCollector:
    """Listen to console and pageerror events while the block runs."""

    def __init__(self, page: Any, *, ignore: Iterable[str] = ()) -> None:
        self._page = page
        self._ignore = tuple(ignore)
        self.errors: list[str] = []

    def _ignored(self, text: str) -> bool:
        return any(pattern in text for pattern in self._ignore)

    def _on_console(self, message: Any) -> None:
        if message.type == "error" and not self._ignored(message.text):
            self.errors.append(message.text)

    def _on_page_error(self, error: Any) -> None:
        text = str(error)
        if not self._ignored(text):
            self.errors.append(text)

    def __enter__(self) -> ConsoleErrorCollector:
        self._page.on("console", self._on_console)
        self._page.on("pageerror", self._on_page_error)
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        self._page.remove_listener("pageerror", self._on_page_error)
        if exc_type is None and self.errors:
            raise AssertionError("console errors: " + "; ".join(self.errors))
