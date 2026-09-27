"""Device profile whose phone, firmware, and fingerprint fields are checked by regex."""

from __future__ import annotations

import re
from typing import Any

from pydantic import BaseModel, ConfigDict, Field, field_validator

PHONE_REGEX = re.compile(r"^\+[1-9]\d{1,10}$")
SEMVER_REGEX = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)"
    r"(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?"
    r"(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
)
FINGERPRINT_REGEX = re.compile(r"^[0-9a-fA-F]{64}$")


class DeviceProfile(BaseModel):
    """A registered device with normalized, validated identifiers."""

    model_config = ConfigDict(extra="forbid")

    device_id: str
    phone: str = Field(..., max_length=16)
    firmware: str
    fingerprint: str

    @field_validator("phone", mode="before")
    @classmethod
    def validate_phone(cls, v: Any) -> str:
        """Strip separators and require an E.164 number."""
        if isinstance(v, str):
            v = re.sub(r"[\s\-\(\)]", "", v)
        if not isinstance(v, str) or not PHONE_REGEX.match(v):
            raise ValueError(f"Invalid E.164 phone number: {v!r}")
        return v

    @field_validator("firmware", mode="after")
    @classmethod
    def validate_firmware(cls, v: str) -> str:
        """Require a SemVer 2.0.0 version string."""
        if not SEMVER_REGEX.match(v):
            raise ValueError(f"Invalid SemVer 2.0.0 firmware string: {v!r}")
        return v

    @field_validator("fingerprint", mode="after")
    @classmethod
    def validate_fingerprint(cls, v: str) -> str:
        """Require 64 hex characters and normalize them to lower case."""
        if not FINGERPRINT_REGEX.match(v):
            raise ValueError(
                f"Fingerprint must be a 64-character hexadecimal string: {v!r}"
            )
        return v.lower()
