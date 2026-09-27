"""Early stopping that snapshots the best model state_dict, torch-style."""

from __future__ import annotations

import copy
import math
from typing import Any


class EarlyStopping:
    """Stop when validation loss has not improved by min_delta for patience epochs."""

    def __init__(self, patience: int = 5, min_delta: float = 0.0) -> None:
        if patience < 1:
            raise ValueError("patience must be at least 1")
        if min_delta < 0:
            raise ValueError("min_delta must be non-negative")
        self.patience = patience
        self.min_delta = min_delta
        self.best_loss = math.inf
        self.best_state: dict[str, Any] | None = None
        self.bad_epochs = 0

    def step(self, val_loss: float, model: Any) -> bool:
        """Record one epoch; return True when training should stop."""
        if math.isnan(val_loss):
            raise ValueError("validation loss is NaN")
        if val_loss < self.best_loss - self.min_delta:
            self.best_loss = val_loss
            self.best_state = copy.deepcopy(model.state_dict())
            return False
        self.bad_epochs += 1
        return self.bad_epochs >= self.patience
