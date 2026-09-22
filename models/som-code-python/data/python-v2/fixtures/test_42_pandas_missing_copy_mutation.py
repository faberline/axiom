"""Oracle test suite for 42-pandas-missing-copy-mutation."""
import pytest
import candidate
from candidate import CohortFeatureTransformer, DataFrame


def test_default_base_scale_transformation():
    transformer = CohortFeatureTransformer()
    df = DataFrame({
        "cohort": ["alpha", "alpha", "beta"],
        "score": [40.0, 60.0, 70.0],
    })
    res = transformer.transform_cohort(df, "alpha")
    # Catches miss_1 where base_scale defaults to 0.0 collapsing scores to zero
    assert res["normalized_score"][0] == 40.0, "Default base_scale=1.0 should preserve score values"
    assert res["normalized_score"][1] == 60.0, "Default base_scale=1.0 should preserve score values"


def test_zero_and_negative_scale_rejected():
    transformer = CohortFeatureTransformer()
    df = DataFrame({"cohort": ["gamma"], "score": [50.0]})
    # Catches miss_2 which omits scale > 0.0 validation
    with pytest.raises(ValueError, match="scale"):
        transformer.transform_cohort(df, "gamma", scale=0.0)
    with pytest.raises(ValueError, match="scale"):
        transformer.transform_cohort(df, "gamma", scale=-1.5)


def test_exact_fifty_threshold_boundary():
    transformer = CohortFeatureTransformer()
    df = DataFrame({
        "cohort": ["delta", "delta"],
        "score": [49.9, 50.0],
    })
    res = transformer.transform_cohort(df, "delta")
    # Catches miss_3 which uses > 50.0 strictly instead of >= 50.0
    assert res["risk_tier"][0] == "standard"
    assert res["risk_tier"][1] == "high", "Score of exactly 50.0 must be categorized into 'high' risk tier"


def test_risk_tier_branch_assignment():
    transformer = CohortFeatureTransformer()
    df = DataFrame({
        "cohort": ["epsilon", "epsilon"],
        "score": [20.0, 80.0],
    })
    res = transformer.transform_cohort(df, "epsilon")
    # Catches miss_4 which inverts risk tier assignment branches
    assert res["risk_tier"][0] == "standard", "Score below 50.0 must be 'standard' tier"
    assert res["risk_tier"][1] == "high", "Score above 50.0 must be 'high' tier"


def test_master_dataframe_isolation():
    transformer = CohortFeatureTransformer()
    df = DataFrame({
        "cohort": ["zeta", "zeta", "eta"],
        "score": [30.0, 65.0, 90.0],
    })
    res = transformer.transform_cohort(df, "zeta")
    assert "normalized_score" in res.columns
    assert "risk_tier" in res.columns
    # Catches miss_5 where omitting .copy() mutates the master DataFrame
    assert "normalized_score" not in df.columns, "Master DataFrame should not be mutated with normalized_score"
    assert "risk_tier" not in df.columns, "Master DataFrame should not be mutated with risk_tier"


def test_missing_cohort_rejected():
    transformer = CohortFeatureTransformer()
    df = DataFrame({"cohort": ["A", "B"], "score": [10.0, 20.0]})
    with pytest.raises(ValueError, match="Cohort"):
        transformer.transform_cohort(df, "nonexistent_cohort")


def test_missing_columns_rejected():
    transformer = CohortFeatureTransformer(cohort_col="team")
    df = DataFrame({"cohort": ["A"], "score": [10.0]})
    with pytest.raises(ValueError, match="Cohort column"):
        transformer.transform_cohort(df, "A")

    transformer_valid = CohortFeatureTransformer(cohort_col="cohort")
    with pytest.raises(ValueError, match="Score column"):
        transformer_valid.transform_cohort(df, "A", score_col="points")
