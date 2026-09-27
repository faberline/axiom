import numpy as np
import pytest
from sklearn.linear_model import LogisticRegression
from sklearn.model_selection import GroupKFold, cross_val_score
from sklearn.neighbors import KNeighborsClassifier

from candidate import CVResult, grouped_cv_score


@pytest.fixture
def patients():
    rng = np.random.default_rng(3)
    n_patients, per = 20, 6
    signature = rng.normal(size=(n_patients, 4))
    label = np.array([0, 1] * (n_patients // 2))
    groups = np.repeat(np.arange(n_patients), per)
    x = signature[groups] + rng.normal(scale=0.01, size=(groups.size, 4))
    y = label[groups]
    order = rng.permutation(groups.size)
    return x[order], y[order], groups[order]


def test_grouping_removes_patient_leakage(patients):
    x, y, groups = patients
    knn = KNeighborsClassifier(n_neighbors=1)
    leaky = cross_val_score(knn, x, y, cv=5).mean()
    assert leaky > 0.95
    result = grouped_cv_score(knn, x, y, groups)
    assert isinstance(result, CVResult)
    assert len(result.scores) == 5
    assert result.mean < 0.8


def test_scores_match_group_kfold_and_sample_std(patients):
    x, y, groups = patients
    model = LogisticRegression()
    expected = cross_val_score(model, x, y, groups=groups, cv=GroupKFold(n_splits=4))
    result = grouped_cv_score(model, x, y, groups, n_splits=4)
    np.testing.assert_allclose(result.scores, expected)
    assert result.mean == pytest.approx(expected.mean())
    assert result.std == pytest.approx(np.std(expected, ddof=1))


def test_exactly_n_splits_groups_is_enough(patients):
    x, y, groups = patients
    keep = groups < 5
    result = grouped_cv_score(LogisticRegression(), x[keep], y[keep], groups[keep])
    assert len(result.scores) == 5


def test_too_few_groups_is_rejected(patients):
    x, y, groups = patients
    keep = groups < 4
    with pytest.raises(ValueError, match="need at least 5 groups, got 4"):
        grouped_cv_score(LogisticRegression(), x[keep], y[keep], groups[keep])


def test_misaligned_groups_are_rejected(patients):
    x, y, groups = patients
    with pytest.raises(ValueError, match="groups must align"):
        grouped_cv_score(LogisticRegression(), x, y, groups[:-1])


def test_model_is_not_fitted_in_place(patients):
    x, y, groups = patients
    model = LogisticRegression()
    grouped_cv_score(model, x, y, groups)
    assert not hasattr(model, "coef_")
