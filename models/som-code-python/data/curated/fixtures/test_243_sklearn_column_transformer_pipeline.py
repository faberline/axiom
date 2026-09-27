import numpy as np
import pandas as pd
import pytest

from candidate import build_model


@pytest.fixture
def frame():
    rng = np.random.default_rng(7)
    n = 60
    df = pd.DataFrame(
        {
            "id": [f"u{i}" for i in range(n)],
            "age": rng.integers(18, 70, n).astype(float),
            "income": rng.normal(50_000, 12_000, n),
            "plan": rng.choice(["free", "pro", "team"], n, p=[0.6, 0.3, 0.1]),
            "country": rng.choice(["tw", "jp", "us"], n),
        }
    )
    y = (df["income"] > 50_000).astype(int)
    df.loc[[1, 2, 3], "age"] = np.nan
    df["plan"] = df["plan"].astype(object)
    df.loc[[4, 5], "plan"] = np.nan
    return df, y


def test_fits_with_missing_values_and_extra_columns(frame):
    df, y = frame
    model = build_model().fit(df, y)
    assert model.score(df, y) > 0.8


def test_numeric_imputer_uses_the_median(frame):
    df, y = frame
    model = build_model().fit(df, y)
    num = model.named_steps["pre"].named_transformers_["num"]
    assert num.named_steps["impute"].statistics_[0] == pytest.approx(df["age"].median())
    scaled = num.transform(df[["age", "income"]])
    np.testing.assert_allclose(scaled.mean(axis=0), 0, atol=1e-9)


def test_missing_categories_become_the_most_frequent(frame):
    df, y = frame
    model = build_model().fit(df, y)
    cat = model.named_steps["pre"].named_transformers_["cat"]
    plans = cat.named_steps["onehot"].categories_[0].tolist()
    assert plans == ["free", "pro", "team"]


def test_unseen_categories_are_ignored_at_predict_time(frame):
    df, y = frame
    model = build_model().fit(df, y)
    new = df.head(3).copy()
    new["country"] = "fr"
    assert model.predict(new).shape == (3,)


def test_regularization_strength_is_passed_through(frame):
    df, y = frame
    model = build_model(c=0.05)
    assert model.named_steps["clf"].C == 0.05
    assert build_model().named_steps["clf"].C == 1.0
    model.fit(df, y)
