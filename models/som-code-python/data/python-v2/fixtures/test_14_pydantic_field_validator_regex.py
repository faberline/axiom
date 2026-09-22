import pytest
from pydantic import ValidationError
from candidate import DeviceProfile


def test_gold_valid_device_profile():
    # Valid profile with clean phone, strict semver, uppercase fingerprint
    profile = DeviceProfile(
        device_id="dev_001",
        phone="+15551234567",
        firmware="2.1.0-alpha.1+build.2024",
        fingerprint="A1B2C3D4" * 8,
    )
    assert profile.device_id == "dev_001"
    assert profile.phone == "+15551234567"
    assert profile.firmware == "2.1.0-alpha.1+build.2024"
    assert profile.fingerprint == ("a1b2c3d4" * 8)


def test_semver_unanchored_trailing_garbage_rejected():
    # Catches miss_1 (missing_validation): unanchored search allows trailing illegal characters
    with pytest.raises(ValidationError) as exc_info:
        DeviceProfile(
            device_id="dev_002",
            phone="+15551234567",
            firmware="1.0.0-rc.1#illegal_suffix",
            fingerprint="a" * 64,
        )
    assert "firmware" in str(exc_info.value)


def test_international_e164_phone_boundaries_accepted():
    # Catches miss_2 (wrong_boundary): restricts digits to {1,10} instead of {6,14}
    # 13 digits total (+49 followed by 11 digits) is standard international format
    profile = DeviceProfile(
        device_id="dev_003",
        phone="+4915123456789",
        firmware="1.0.0",
        fingerprint="a" * 64,
    )
    assert profile.phone == "+4915123456789"


def test_formatted_phone_mode_before_precleaning():
    # Catches miss_3 (wrong_api_call): uses mode="after" on phone validator
    # Formatted input has 18 raw characters; pre-cleaning must occur before max_length=16 check
    profile = DeviceProfile(
        device_id="dev_004",
        phone="+1 (555) 019-2834",
        firmware="1.0.0",
        fingerprint="a" * 64,
    )
    assert profile.phone == "+15550192834"
    # Also verify decorator configuration is explicitly mode='before'
    validator_info = DeviceProfile.__pydantic_decorators__.field_validators["validate_phone"].info
    assert validator_info.mode == "before", f"Expected mode='before', got {validator_info.mode!r}"


def test_fingerprint_lowercase_normalization():
    # Catches miss_4 (wrong_branch): omits .lower() on fingerprint, preserving uppercase
    raw_hex = "A1B2C3D4E5F67890" * 4
    profile = DeviceProfile(
        device_id="dev_005",
        phone="+15551234567",
        firmware="1.0.0",
        fingerprint=raw_hex,
    )
    assert profile.fingerprint == raw_hex.lower(), f"Expected lowercase fingerprint, got {profile.fingerprint!r}"
    assert not any(c.isupper() for c in profile.fingerprint), "Fingerprint must not contain uppercase letters"


def test_invalid_semver_raises_validation_error():
    # Catches miss_5 (wrong_default): falls back to '0.0.0' on invalid SemVer instead of raising ValidationError
    with pytest.raises(ValidationError) as exc_info:
        DeviceProfile(
            device_id="dev_006",
            phone="+15551234567",
            firmware="not_a_valid_semver",
            fingerprint="a" * 64,
        )
    assert "firmware" in str(exc_info.value)


def test_invalid_phone_formats_rejected():
    for bad_phone in ["1234567890", "+0123456789", "+1234", "not-a-phone"]:
        with pytest.raises(ValidationError):
            DeviceProfile(
                device_id="dev_bad",
                phone=bad_phone,
                firmware="1.0.0",
                fingerprint="a" * 64,
            )


def test_invalid_fingerprint_rejected():
    with pytest.raises(ValidationError):
        DeviceProfile(
            device_id="dev_bad_fp",
            phone="+15551234567",
            firmware="1.0.0",
            fingerprint="short_hex",
        )
