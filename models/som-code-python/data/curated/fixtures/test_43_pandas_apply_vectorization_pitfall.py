"""Oracle test suite for 43-pandas-apply-vectorization-pitfall."""
import pytest
import candidate
from candidate import TransactionVectorizedScorer, DataFrame, Series


def test_gold_vectorized_scoring_accuracy():
    scorer = TransactionVectorizedScorer()
    df = DataFrame({
        "amount": [500.0, 2000.0, 3000.0],
        "frequency": [4, 10, 20],
    })
    # Low: 500*0.01 + 4*0.5 = 5.0 + 2.0 = 7.0
    # High: 2000*0.05 + 10*1.5 = 100.0 + 15.0 = 115.0 -> capped at 100.0
    # High: 3000*0.05 + 20*1.5 = 150.0 + 30.0 = 180.0 -> capped at 100.0
    scores = scorer.score_transactions(df)
    assert scores[0] == pytest.approx(7.0)
    assert scores[1] == pytest.approx(100.0)
    assert scores[2] == pytest.approx(100.0)


def test_default_max_cap_allows_above_fifty():
    scorer = TransactionVectorizedScorer()
    df = DataFrame({
        "amount": [1200.0],
        "frequency": [10],
    })
    # 1200*0.05 + 10*1.5 = 60.0 + 15.0 = 75.0
    scores = scorer.score_transactions(df)
    # Catches miss_1 with max_cap=50.0 prematurely truncating scores
    assert scores[0] == pytest.approx(75.0), "Default max_cap should allow scores up to 100.0"


def test_negative_weight_rejected():
    # Catches miss_2 which omits weight > 0.0 validation
    with pytest.raises(ValueError, match="weight"):
        TransactionVectorizedScorer(weight=-1.0)
    with pytest.raises(ValueError, match="weight"):
        TransactionVectorizedScorer(weight=0.0)


def test_exact_thousand_boundary_tier():
    scorer = TransactionVectorizedScorer()
    df = DataFrame({
        "amount": [1000.0],
        "frequency": [4],
    })
    # Amount 1000.0 should use standard tier (<= 1000.0):
    # 1000.0 * 0.01 + 4 * 0.5 = 10.0 + 2.0 = 12.0
    # miss_3 uses >= 1000.0: 1000.0 * 0.05 + 4 * 1.5 = 50.0 + 6.0 = 56.0
    scores = scorer.score_transactions(df)
    assert scores[0] == pytest.approx(12.0), "Amount 1000.0 must be categorized in the standard tier"


def test_branch_logic_high_vs_low_amounts():
    scorer = TransactionVectorizedScorer()
    df = DataFrame({
        "amount": [500.0, 1500.0],
        "frequency": [2, 2],
    })
    # Low: 500*0.01 + 2*0.5 = 5.0 + 1.0 = 6.0
    # High: 1500*0.05 + 2*1.5 = 75.0 + 3.0 = 78.0
    # miss_4 inverts branches: Low gives 28.0, High gives 16.0
    scores = scorer.score_transactions(df)
    assert scores[0] == pytest.approx(6.0), "Low tier calculation incorrect"
    assert scores[1] == pytest.approx(78.0), "High tier calculation incorrect"


def test_custom_index_preservation_and_vectorized_api():
    scorer = TransactionVectorizedScorer()
    custom_index = ["tx_alpha", "tx_beta"]
    df = DataFrame(
        {"amount": [400.0, 1200.0], "frequency": [2, 4]},
        index=custom_index,
    )
    scores = scorer.score_transactions(df)
    # Catches miss_5 which uses unvectorized apply losing index
    assert list(scores.index) == custom_index, "Returned Series must preserve the input DataFrame index"
    assert scores["tx_alpha"] == pytest.approx(5.0)
    assert scores["tx_beta"] == pytest.approx(66.0)


def test_empty_dataframe_rejected():
    scorer = TransactionVectorizedScorer()
    df = DataFrame({"amount": [], "frequency": []})
    with pytest.raises(ValueError, match="empty"):
        scorer.score_transactions(df)


def test_missing_columns_rejected():
    scorer = TransactionVectorizedScorer()
    df = DataFrame({"amount": [100.0]})
    with pytest.raises(ValueError, match="frequency"):
        scorer.score_transactions(df, freq_col="frequency")

    df_nofreq = DataFrame({"frequency": [1]})
    with pytest.raises(ValueError, match="amount"):
        scorer.score_transactions(df_nofreq, amount_col="amount")
