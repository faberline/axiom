"""Playwright browser and context lifecycle manager preventing zombie processes."""
from typing import Any, Callable, Optional


class ManagedBrowserSession:
    """Manages browser and context lifecycle with guaranteed cleanup in try/finally."""

    def __init__(
        self,
        launcher: Optional[Any] = None,
        auto_context: bool = True,
    ) -> None:
        self.launcher = launcher
        self.auto_context = auto_context
        self.browser: Optional[Any] = None
        self.context: Optional[Any] = None
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
        if self._is_closed:
            return

        try:
            if self.context is not None:
                self.context.close()
        finally:
            self.context = None
            try:
                if self.browser is not None:
                    self.browser.close()
            finally:
                self.browser = None
                self._is_closed = True

    def __enter__(self) -> Any:
        return self.open()

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()

    def run_scoped(self, action: Callable[[Any], Any]) -> Any:
        """Execute action within managed session, guaranteeing cleanup on any error."""
        session = self.open()
        try:
            return action(session)
        finally:
            self.close()
