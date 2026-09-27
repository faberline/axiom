import warnings

import numpy as np
import pytest

from candidate import top_k_similar

CORPUS = np.array(
    [[1.0, 0.0], [0.0, 1.0], [1.0, 1.0], [-1.0, 0.0], [0.0, 0.0], [2.0, 0.1]]
)


def test_top_rows_by_cosine_not_dot_product():
    idx, scores = top_k_similar([1.0, 0.0], CORPUS, 3)
    assert idx.tolist() == [0, 5, 2]
    np.testing.assert_allclose(scores, [1.0, 2 / np.hypot(2, 0.1), 0.5**0.5])


def test_zero_rows_score_zero_without_warnings():
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        idx, scores = top_k_similar([1.0, 0.0], CORPUS, 6)
    assert not np.isnan(scores).any()
    assert scores[idx.tolist().index(4)] == 0.0


def test_k_larger_than_corpus_returns_everything_sorted():
    idx, scores = top_k_similar([1.0, 0.0], CORPUS, 50)
    assert sorted(idx.tolist()) == list(range(6))
    assert (np.diff(scores) <= 0).all()
    assert idx[-1] == 3


def test_invalid_inputs_are_rejected():
    with pytest.raises(ValueError, match="zero"):
        top_k_similar([0.0, 0.0], CORPUS, 2)
    with pytest.raises(ValueError, match="columns"):
        top_k_similar([1.0, 0.0, 0.0], CORPUS, 2)
    with pytest.raises(ValueError, match="positive"):
        top_k_similar([1.0, 0.0], CORPUS, 0)
