import numpy as np
import pytest
import candidate


def test_gold_rescaling_and_clipping():
    rescaler = candidate.SafeImagePixelRescaler(target_dtype="uint8")
    img = np.array([[10, 100], [200, 250]], dtype=np.uint8)
    out = rescaler.rescale_intensity(img, scale=1.5, offset=10.0)
    assert out.dtype == np.uint8
    expected = np.array([[25, 160], [255, 255]], dtype=np.uint8)
    assert np.array_equal(out, expected)


def test_default_target_dtype_is_uint8():
    rescaler = candidate.SafeImagePixelRescaler()
    assert rescaler.target_dtype == "uint8"
    img = np.array([50, 150, 200], dtype=np.uint8)
    out = rescaler.rescale_intensity(img)
    assert out.dtype == np.uint8
    assert np.all(out >= 0)


def test_negative_or_zero_scale_rejected():
    rescaler = candidate.SafeImagePixelRescaler()
    img = np.array([10, 20], dtype=np.uint8)
    with pytest.raises(ValueError, match="scale"):
        rescaler.rescale_intensity(img, scale=0.0)
    with pytest.raises(ValueError, match="scale"):
        rescaler.rescale_intensity(img, scale=-2.0)


def test_clip_upper_bound_boundary_does_not_wrap():
    rescaler = candidate.SafeImagePixelRescaler(target_dtype="uint8")
    img = np.array([200], dtype=np.uint8)
    out = rescaler.rescale_intensity(img, scale=1.5)
    assert out[0] == 255


def test_target_dtype_branch_limits():
    rescaler_u8 = candidate.SafeImagePixelRescaler(target_dtype="uint8")
    img = np.array([200], dtype=np.uint8)
    out_u8 = rescaler_u8.rescale_intensity(img, scale=2.0)
    assert out_u8[0] == 255

    rescaler_u16 = candidate.SafeImagePixelRescaler(target_dtype="uint16")
    img_u16 = np.array([50000], dtype=np.uint16)
    out_u16 = rescaler_u16.rescale_intensity(img_u16, scale=1.5)
    assert out_u16.dtype == np.uint16
    assert out_u16[0] == 65535


def test_no_silent_intermediate_overflow():
    rescaler = candidate.SafeImagePixelRescaler(target_dtype="uint8")
    img = np.array([150, 200], dtype=np.uint8)
    out = rescaler.rescale_intensity(img, scale=2, offset=0)
    assert np.array_equal(out, np.array([255, 255], dtype=np.uint8))


def test_parameter_validations():
    with pytest.raises(ValueError, match="target_dtype"):
        candidate.SafeImagePixelRescaler(target_dtype="int32")
    rescaler = candidate.SafeImagePixelRescaler()
    with pytest.raises(TypeError, match="numpy ndarray"):
        rescaler.rescale_intensity([[1, 2], [3, 4]])
