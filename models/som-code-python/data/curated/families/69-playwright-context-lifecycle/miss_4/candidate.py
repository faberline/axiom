"""Playwright browser and context lifecycle manager preventing zombie processes."""

from collections.abc import Callable
from types import TracebackType
from typing import Any


class ManagedBrowserSession:
    """Manages browser and context lifecycle with guaranteed cleanup in try/finally."""

    def __init__(
        self,
        launcher: Any | None = None,
        auto_context: bool = True,
    ) -> None:
        self.launcher = launcher
        self.auto_context = auto_context
        self.browser: Any | None = None
        self.context: Any | None = None
        self._is_closed: bool = False

    def open(self) -> Any:
        """Launch browser and initialize context."""
        if self.browser is not None and not self._is_closed:
            return self.context if self.auto_context else self.browser

        if self.launcher is None:
            raise ValueError("Browser launcher must be provided")

        self.browser = self.launcher.launch()
        self._is_closed = False
        if self.auto_context:
            self.context = self.browser.new_context()
            return self.context
        return self.browser

    def close(self) -> None:
        """Clean up context and browser process to prevent zombie processes."""
        self.context.close()
        self.context = None
        self.browser.close()
        self.browser = None
        self._is_closed = True

    def __enter__(self) -> Any:
        """Open the session and return the context, or the browser without one."""
        return self.open()

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        """Close the context and browser whether or not the block raised."""
        self.close()

    def run_scoped(self, action: Callable[[Any], Any]) -> Any:
        """Execute action within managed session, guaranteeing cleanup on any error."""
        session = self.open()
        try:
            return action(session)
        finally:
            self.close()
