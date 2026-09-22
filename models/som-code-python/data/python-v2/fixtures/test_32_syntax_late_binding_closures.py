import pytest
from candidate import RulePipeline


def test_scalers_early_binding():
    pipeline = RulePipeline()
    scalers = pipeline.build_indexed_scalers([2, 3, 5])
    assert [s(10) for s in scalers] == [20, 30, 50]


def test_empty_factors_validation():
    pipeline = RulePipeline()
    with pytest.raises(ValueError, match="factors cannot be empty"):
        pipeline.build_indexed_scalers([])


def test_thresholds_boundary_twenty_allowed():
    pipeline = RulePipeline()
    thresh = [float(i) for i in range(20)]
    checkers = pipeline.register_threshold_checks(thresh)
    assert len(checkers) == 20


def test_evaluate_mode_all():
    pipeline = RulePipeline()
    pipeline.register_threshold_checks([10.0, 20.0])
    assert pipeline.evaluate(15.0, mode="all") is False
    assert pipeline.evaluate(25.0, mode="all") is True


def test_clear_resets_history_and_rules():
    pipeline = RulePipeline()
    pipeline.register_threshold_checks([5.0])
    pipeline.evaluate(10.0)
    assert pipeline.history_count() == 1
    pipeline.clear()
    assert pipeline.history_count() == 0
    assert len(pipeline.rules) == 0


def test_type_validations():
    pipeline = RulePipeline()
    with pytest.raises(TypeError):
        pipeline.register_threshold_checks("not_a_list")
    with pytest.raises(TypeError):
        pipeline.build_indexed_scalers("not_a_list")
    with pytest.raises(ValueError):
        pipeline.evaluate(10.0, mode="invalid_mode")
