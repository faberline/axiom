"""SOM: local decisions over a caller-provided candidate list."""

from __future__ import annotations

from typing import Any

__all__ = ("SOMPredictor",)


class SOMPredictor:
    """Lightweight public facade for the local SOM predictor.

    Importing this class never starts a numerical runtime. The implementation
    loads only when :meth:`predict` is called.
    """

    def __init__(self, checkpoint_root: str | None = None):
        self._checkpoint_root = checkpoint_root
        self._implementation: Any | None = None

    def _predictor(self) -> Any:
        if self._implementation is None:
            from .specialists.predict import SpecialistPredictor

            self._implementation = SpecialistPredictor(self._checkpoint_root)
        return self._implementation

    def predict(self, request: dict[str, Any], domain: str = "frontend") -> dict[str, Any]:
        """Rank caller-provided candidates without executing their code."""
        return self._predictor().predict(request, domain=domain)
