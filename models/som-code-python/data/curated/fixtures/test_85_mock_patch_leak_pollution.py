import pytest
from candidate import RemoteClockService, run_with_patched_clock, verify_clock_unpolluted

def test_successful_run_restores_original_target():
    def sample_test():
        assert RemoteClockService.get_timestamp() == 5555
        return "ok"

    res = run_with_patched_clock(5555, sample_test)
    assert res == "ok"
    assert verify_clock_unpolluted() is True
    assert RemoteClockService.get_timestamp() == 1000

def test_failing_test_restores_original_target():
    def failing_test():
        assert RemoteClockService.get_timestamp() == 9999
        raise ValueError("test assertion failed")

    with pytest.raises(ValueError, match="test assertion failed"):
        run_with_patched_clock(9999, failing_test)
    assert verify_clock_unpolluted() is True
    assert RemoteClockService.get_timestamp() == 1000
