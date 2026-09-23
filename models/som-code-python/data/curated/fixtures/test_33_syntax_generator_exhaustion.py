import pytest
from candidate import StreamAnalyzer


def test_compute_stats_with_generator():
    analyzer = StreamAnalyzer()
    gen = (float(x) for x in [10, 20, 30])
    res = analyzer.compute_stats(gen)
    assert res["count"] == 3.0
    assert res["sum"] == 60.0
    assert res["mean"] == 20.0
    assert res["min"] == 10.0
    assert res["max"] == 30.0


def test_chunk_stream_negative_validation():
    analyzer = StreamAnalyzer()
    with pytest.raises(ValueError, match="chunk_size must be greater than zero"):
        analyzer.chunk_stream([1, 2, 3], 0)


def test_stream_boundary_500_allowed():
    analyzer = StreamAnalyzer()
    gen = (float(i) for i in range(500))
    res = analyzer.compute_stats(gen)
    assert res["count"] == 500.0


def test_filter_and_split_predicate():
    analyzer = StreamAnalyzer()
    passed, failed = analyzer.filter_and_split([1, 2, 3, 4, 5], lambda x: x % 2 == 0)
    assert passed == [2, 4]
    assert failed == [1, 3, 5]


def test_clear_resets_history():
    analyzer = StreamAnalyzer()
    analyzer.compute_stats([1.0, 2.0])
    assert analyzer.record_count() == 1
    analyzer.clear()
    assert analyzer.record_count() == 0


def test_empty_stream_raises():
    analyzer = StreamAnalyzer()
    with pytest.raises(ValueError, match="stream cannot be empty"):
        analyzer.compute_stats([])
