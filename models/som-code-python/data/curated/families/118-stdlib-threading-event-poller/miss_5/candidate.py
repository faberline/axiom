"""Run a background polling worker that stops promptly through threading.Event."""

import threading
from collections.abc import Callable


class Poller:
    """Call a task every interval seconds on a daemon thread until stopped."""

    def __init__(self, task: Callable[[], None], interval: float) -> None:
        if interval <= 0:
            raise ValueError("interval must be positive")
        self._task = task
        self._interval = interval
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self.errors: list[Exception] = []

    def start(self) -> None:
        """Start the worker thread exactly once."""
        if self._thread is not None:
            raise RuntimeError("poller already started")
        self._thread = threading.Thread(target=self._run, name="poller", daemon=True)
        self._thread.start()

    def _run(self) -> None:
        while not self._stop.is_set():
            try:
                self._task()
            except Exception as exc:  # pylint: disable=broad-exception-caught
                self.errors.append(exc)
            self._stop.wait(self._interval)

    def stop(self, timeout: float = 1.0) -> bool:
        """Signal the worker and report whether it exited within timeout."""
        if self._thread is None:
            return True
        self._thread.join(timeout)
        self._stop.set()
        return not self._thread.is_alive()
