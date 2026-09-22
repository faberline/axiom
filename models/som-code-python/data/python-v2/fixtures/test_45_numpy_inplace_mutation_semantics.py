import numpy as np
import pytest
import candidate


def test_gold_signal_processing():
    proc = candidate.SignalGainProcessor(default_gain=2.5)
    sig = np.array([-1.0, 0.0, 1.0, 2.0], dtype=np.float64)
    out = proc.process_signal(sig, threshold=0.5)
    assert np.allclose(out, np.array([-1.0, 0.0, 2.5, 5.0], dtype=np.float64))
    assert np.allclose(sig, np.array([-1.0, 0.0, 1.0, 2.0], dtype=np.float64))


def test_out_of_place_preserves_input_by_default():
    proc = candidate.SignalGainProcessor()
    sig = np.array([1.0, 2.0, 3.0], dtype=np.float64)
    sig_copy = sig.copy()
    out = proc.process_signal(sig)
    assert np.array_equal(sig, sig_copy)
    assert not np.shares_memory(sig, out)


def test_rejects_integer_dtype_signal():
    proc = candidate.SignalGainProcessor()
    sig_int = np.array([1, 2, 3], dtype=np.int32)
    with pytest.raises(TypeError, match="floating"):
        proc.process_signal(sig_int)
    sig_int64 = np.array([1, 2, 3], dtype=np.int64)
    with pytest.raises(TypeError, match="floating"):
        proc.process_signal(sig_int64)


def test_exact_threshold_value_receives_gain():
    proc = candidate.SignalGainProcessor()
    sig = np.array([1.0, 1.5, 2.0], dtype=np.float64)
    out = proc.process_signal(sig, gain=2.0, threshold=1.5)
    assert out[1] == 3.0
    assert np.array_equal(out, np.array([1.0, 3.0, 4.0], dtype=np.float64))


def test_gating_branch_direction():
    proc = candidate.SignalGainProcessor()
    sig = np.array([-2.0, 2.0], dtype=np.float64)
    out = proc.process_signal(sig, gain=3.0, threshold=0.0)
    assert out[0] == -2.0
    assert out[1] == 6.0


def test_explicit_in_place_false_does_not_share_memory():
    proc = candidate.SignalGainProcessor()
    sig = np.array([0.5, 1.5, 2.5], dtype=np.float64)
    out = proc.process_signal(sig, gain=4.0, threshold=1.0, in_place=False)
    assert not np.shares_memory(sig, out)
    assert sig[1] == 1.5
    assert out[1] == 6.0


def test_explicit_in_place_true_mutates_input():
    proc = candidate.SignalGainProcessor()
    sig = np.array([0.5, 1.5, 2.5], dtype=np.float64)
    out = proc.process_signal(sig, gain=2.0, threshold=1.0, in_place=True)
    assert np.shares_memory(sig, out)
    assert np.array_equal(sig, np.array([0.5, 3.0, 5.0], dtype=np.float64))


def test_parameter_validations():
    with pytest.raises(ValueError, match="gain"):
        candidate.SignalGainProcessor(default_gain=-1.0)
    with pytest.raises(ValueError, match="gain"):
        candidate.SignalGainProcessor(default_gain=0.0)
    proc = candidate.SignalGainProcessor()
    with pytest.raises(TypeError, match="numpy ndarray"):
        proc.process_signal([1.0, 2.0])
    with pytest.raises(ValueError, match="gain"):
        proc.process_signal(np.array([1.0, 2.0]), gain=-0.5)
