"""Tune a scaled logistic regression with a seeded, stratified grid search."""

from __future__ import annotations

import numpy as np
from numpy.typing import ArrayLike
from sklearn.linear_model import LogisticRegression
from sklearn.model_selection import GridSearchCV, StratifiedKFold
from sklearn.pipeline import Pipeline
from sklearn.preprocessing import StandardScaler

N_SPLITS = 5
PARAM_GRID = {"clf__C": [0.01, 0.1, 1.0, 10.0]}


def tune(x: ArrayLike, y: ArrayLike, *, seed: int = 0) -> GridSearchCV:
    """Grid-search C by macro F1 and refit the best pipeline on all data."""
    _, counts = np.unique(np.asarray(y), return_counts=True)
    if counts.size < 2 or counts.min() < N_SPLITS:
        raise ValueError(f"need two or more classes with {N_SPLITS}+ samples each")
    pipe = Pipeline(
        [("scale", StandardScaler()), ("clf", LogisticRegression(max_iter=1000))]
    )
    cv = StratifiedKFold(n_splits=N_SPLITS, shuffle=True, random_state=seed)
    search = GridSearchCV(pipe, PARAM_GRID, scoring="f1_macro", cv=cv, refit=True)
    return search.fit(x, y)
