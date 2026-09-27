"""Amplify a float signal above a threshold without mutating the caller's array."""

import numpy as np


class SignalGainProcessor:
    """Apply a gain to the samples of a signal at or above a threshold."""

    def __init__(self, default_gain: float = 2.0) -> None:
        if default_gain <= 0.0:
            raise ValueError("default_gain must be strictly positive")
        self.default_gain = float(default_gain)

    def process_signal(
        self,
        signal: np.ndarray,
        gain: float | None = None,
        threshold: float = 0.0,
        in_place: bool = False,
    ) -> np.ndarray:
        """Return the signal with gain applied where it meets the threshold."""
        if not isinstance(signal, np.ndarray):
            raise TypeError("signal must be a numpy ndarray")
        if not np.issubdtype(signal.dtype, np.floating):
            raise TypeError("signal must have floating-point dtype")

        eff_gain = float(gain) if gain is not None else self.default_gain
        if eff_gain <= 0.0:
            raise ValueError("gain must be strictly positive")

        out = signal if in_place else signal.copy()
        mask = out < threshold
        out[mask] = out[mask] * eff_gain
        return out
