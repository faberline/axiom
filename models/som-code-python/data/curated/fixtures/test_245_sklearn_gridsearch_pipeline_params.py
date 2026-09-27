import numpy as np
import pytest
from sklearn.datasets import make_classification
from sklearn.metrics import f1_score
from sklearn.model_selection import StratifiedKFold
from sklearn.pipeline import Pipeline

from candidate import PARAM_GRID, tune


@pytest.fixture
def data():
    x, y = make_classification(
        n_samples=150, n_features=6, weights=[0.8, 0.2], random_state=1
    )
    return x * 1000, y


def test_refits_the_best_pipeline(data):
    x, y = data
    search = tune(x, y)
    best = search.best_estimator_
    assert isinstance(best, Pipeline)
    assert list(best.named_steps) == ["scale", "clf"]
    assert best.named_steps["clf"].C == search.best_params_["clf__C"]
    assert search.best_params_["clf__C"] in PARAM_GRID["clf__C"]
    assert best.predict(x).shape == y.shape


def test_scores_by_macro_f1(data):
    x, y = data
    search = tune(x, y)
    assert search.scoring == "f1_macro"
    idx = search.best_index_
    split = search.cv.split(x, y)
    scores = []
    for train, test in split:
        model = search.estimator.set_params(**search.best_params_)
        model.fit(x[train], y[train])
        scores.append(f1_score(y[test], model.predict(x[test]), average="macro"))
    assert search.cv_results_["mean_test_score"][idx] == pytest.approx(np.mean(scores))


def test_folds_are_stratified_shuffled_and_seeded(data):
    x, y = data
    search = tune(x, y, seed=11)
    assert isinstance(search.cv, StratifiedKFold)
    ours = [t.tolist() for _, t in search.cv.split(x, y)]
    ref = StratifiedKFold(n_splits=5, shuffle=True, random_state=11)
    assert ours == [t.tolist() for _, t in ref.split(x, y)]


def test_rare_or_single_class_is_rejected(data):
    x, y = data
    with pytest.raises(ValueError, match="need two or more classes"):
        tune(x[:20], np.zeros(20, dtype=int))
    y_rare = np.zeros(len(y), dtype=int)
    y_rare[:4] = 1
    with pytest.raises(ValueError, match="need two or more classes"):
        tune(x, y_rare)
