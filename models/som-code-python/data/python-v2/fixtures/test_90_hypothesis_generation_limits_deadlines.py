"""Oracle test suite for 90-hypothesis-generation-limits-deadlines."""
import pytest
import candidate
from candidate import has_path, get_test_settings


def test_gold_settings_and_path_behavior():
    """Verify settings compliance and graph traversal correctness."""
    settings = get_test_settings()
    assert settings["max_examples"] >= 50
    assert settings["deadline_ms"] is not None and settings["deadline_ms"] <= 1000
    assert settings["suppressed_health_checks"] == []

    graph = {1: [2], 2: [3], 3: [4], 4: []}
    assert has_path(graph, 1, 4) is True
    assert has_path(graph, 4, 1) is False
    assert has_path(graph, 1, 1) is True


def test_catches_suppressed_health_checks_wrong_default():
    """Catches miss_1: suppressing too_slow masks algorithmic degradation."""
    settings = get_test_settings()
    assert "too_slow" not in settings.get("suppressed_health_checks", [])
    assert settings.get("suppressed_health_checks") == []


def test_catches_missing_validation_infinite_deadline():
    """Catches miss_2: deadline_ms=None allows infinite runtime."""
    settings = get_test_settings()
    assert settings.get("deadline_ms") is not None


def test_catches_wrong_boundary_trivial_max_examples():
    """Catches miss_3: max_examples=1 trivializes property testing."""
    settings = get_test_settings()
    assert settings.get("max_examples", 0) >= 50


def test_catches_wrong_api_call_multiple_suppressions():
    """Catches miss_4: suppressing multiple health checks."""
    settings = get_test_settings()
    assert len(settings.get("suppressed_health_checks", [])) == 0


def test_catches_wrong_branch_path_skip():
    """Catches miss_5: inverted conditional skips unvisited nodes and fails to find path."""
    graph = {1: [2], 2: [3], 3: [4]}
    assert has_path(graph, 1, 4) is True
