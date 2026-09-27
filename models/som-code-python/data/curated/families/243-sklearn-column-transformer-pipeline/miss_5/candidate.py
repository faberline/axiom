"""A leak-free mixed-type classification pipeline built on ColumnTransformer."""

from __future__ import annotations

from sklearn.compose import ColumnTransformer
from sklearn.impute import SimpleImputer
from sklearn.linear_model import LogisticRegression
from sklearn.pipeline import Pipeline
from sklearn.preprocessing import OneHotEncoder, StandardScaler

NUMERIC = ["age", "income"]
CATEGORICAL = ["plan", "country"]


def build_model(*, c: float = 1.0) -> Pipeline:
    """Impute and scale numbers, impute and one-hot categories, then classify."""
    numeric = Pipeline(
        [("impute", SimpleImputer(strategy="median")), ("scale", StandardScaler())]
    )
    categorical = Pipeline(
        [
            ("impute", SimpleImputer(strategy="most_frequent")),
            ("onehot", OneHotEncoder(handle_unknown="ignore")),
        ]
    )
    pre = ColumnTransformer(
        [("num", numeric, NUMERIC), ("cat", categorical, CATEGORICAL)],
        remainder="drop",
    )
    return Pipeline([("pre", pre), ("clf", LogisticRegression(max_iter=1000))])
