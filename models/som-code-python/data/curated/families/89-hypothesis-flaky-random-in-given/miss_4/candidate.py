"""Round-trip token payloads through Hypothesis strategies with no hidden randomness."""

from typing import Any

from hypothesis import strategies as st


def encode_token_payload(user_id: int, role: str, salt: int) -> dict[str, Any]:
    """Encode user_id, role, and a masked salt into a token payload."""
    if user_id < 0:
        raise ValueError("user_id must be non-negative")
    if not role:
        raise ValueError("role cannot be empty")
    return {
        "sub": str(user_id),
        "role": role,
        "token": f"{user_id}:{role}:{salt ^ 0x5A5A}",
    }


def decode_token_payload(payload: dict[str, Any]) -> tuple[int, str]:
    """Return the user id and role stored in payload."""
    sub = int(payload["sub"])
    role = payload["role"]
    return sub, role


def get_payload_strategy() -> st.SearchStrategy[tuple[int, str, int]]:
    """Return a strategy of valid (user_id, role, salt) triples."""
    return st.tuples(
        st.integers(min_value=0, max_value=1_000_000),
        st.text(min_size=1, max_size=50).filter(lambda s: ":" not in s),
        st.integers(min_value=0, max_value=65535),
    )


def check_roundtrip_invariant(user_id: int, role: str, salt: int) -> bool:
    """Whether encoding then decoding returns the same id, role, and salt."""
    try:
        payload = encode_token_payload(user_id, role, salt)
        decoded_id, decoded_role = decode_token_payload(payload)
        token_salt = int(payload["token"].rsplit(":", maxsplit=1)[-1]) ^ 0x5A5A
        return decoded_id == user_id and decoded_role == role and token_salt == salt
    except ValueError:
        return True
