from typing import Any, Callable, Dict, Tuple

try:
    from hypothesis import strategies as st, given
except ImportError:
    class _TextStrategy:
        def __init__(self, min_size: int = 0, max_size: int = 100) -> None:
            self.min_size = min_size
            self.max_size = max_size
            self.filter_predicate: Callable[[str], bool] | None = None

        def filter(self, predicate: Callable[[str], bool]) -> "_TextStrategy":
            self.filter_predicate = predicate
            return self

        def example(self) -> str:
            val = "user"
            if self.filter_predicate and not self.filter_predicate(val):
                val = "admin"
            return val

    class _IntStrategy:
        def __init__(self, min_value: int = 0, max_value: int = 100) -> None:
            self.min_value = min_value
            self.max_value = max_value

        def example(self) -> int:
            return self.min_value

    class _TupleStrategy:
        def __init__(self, *strategies: Any) -> None:
            self.strategies = strategies

        def example(self) -> Tuple[Any, ...]:
            return tuple(
                s.example() if hasattr(s, "example") else 0
                for s in self.strategies
            )

    class _StrategiesShim:
        def integers(self, min_value: int = 0, max_value: int = 100) -> _IntStrategy:
            return _IntStrategy(min_value, max_value)

        def text(self, min_size: int = 0, max_size: int = 100) -> _TextStrategy:
            return _TextStrategy(min_size, max_size)

        def tuples(self, *strategies: Any) -> _TupleStrategy:
            return _TupleStrategy(*strategies)

    st = _StrategiesShim()  # type: ignore

    def given(*args: Any, **kwargs: Any) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
        def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
            return fn
        return decorator

def encode_token_payload(user_id: int, role: str, salt: int) -> Dict[str, Any]:
    if user_id < 0:
        raise ValueError("user_id must be non-negative")
    if not role:
        raise ValueError("role cannot be empty")
    return {
        "sub": str(user_id),
        "role": role,
        "token": f"{user_id}:{role}:{salt ^ 0x5A5A}",
    }

def decode_token_payload(payload: Dict[str, Any]) -> Tuple[int, str]:
    sub = int(payload["sub"])
    role = payload["role"]
    return sub, role

def get_payload_strategy() -> Any:
    return st.tuples(
        st.integers(min_value=0, max_value=1_000_000),
        st.text(min_size=1, max_size=50),
        st.integers(min_value=0, max_value=65535),
    )

def check_roundtrip_invariant(user_id: int, role: str, salt: int) -> bool:
    payload = encode_token_payload(user_id, role, salt)
    decoded_id, decoded_role = decode_token_payload(payload)
    token_salt = int(payload["token"].split(":")[-1]) ^ 0x5A5A
    return decoded_id == user_id and decoded_role == role and token_salt == salt
