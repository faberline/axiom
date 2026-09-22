import multiprocessing as mp
import pytest
from candidate import ParallelMetricsAggregator


def test_parallel_simulation_consistency():
    agg = ParallelMetricsAggregator(num_categories=4)
    metrics = agg.run_parallel_simulation(workers=4, iterations_per_worker=50)
    assert metrics["total"] == 200, f"Expected total 200, got {metrics['total']}"
    assert sum(metrics["categories"]) == 200
    for cat_val in metrics["categories"]:
        assert cat_val == 50


def test_race_condition_locks_acquired():
    agg = ParallelMetricsAggregator(num_categories=2)
    total_lock_entered = False
    cat_lock_entered = False

    class MonitoredLock:
        def __init__(self, real_lock):
            self.real_lock = real_lock

        def __enter__(self):
            nonlocal total_lock_entered
            total_lock_entered = True
            return self.real_lock.__enter__()

        def __exit__(self, *args):
            return self.real_lock.__exit__(*args)

    class MonitoredCatLock:
        def __init__(self, real_lock):
            self.real_lock = real_lock

        def __enter__(self):
            nonlocal cat_lock_entered
            cat_lock_entered = True
            return self.real_lock.__enter__()

        def __exit__(self, *args):
            return self.real_lock.__exit__(*args)

    orig_total_get_lock = agg.total_count.get_lock
    orig_cat_get_lock = agg.category_counts.get_lock

    agg.total_count.get_lock = lambda: MonitoredLock(orig_total_get_lock())
    agg.category_counts.get_lock = lambda: MonitoredCatLock(orig_cat_get_lock())

    agg.record_batch(0, 5)
    assert total_lock_entered is True, "total_count.get_lock() was not acquired"
    assert cat_lock_entered is True, "category_counts.get_lock() was not acquired"


def test_category_bounds_validation():
    agg = ParallelMetricsAggregator(num_categories=5)
    with pytest.raises(IndexError):
        agg.record_batch(10, 1)

    with pytest.raises(IndexError):
        agg.record_batch(-1, 1)
