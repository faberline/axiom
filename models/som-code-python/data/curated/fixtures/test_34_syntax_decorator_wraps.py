import pytest
from candidate import HandlerRegistry, audit_logged


def test_audit_logged_preserves_metadata():
    @audit_logged("compute")
    def add_numbers(a: int, b: int) -> int:
        """Add two numbers together."""
        return a + b

    assert add_numbers(2, 3) == 5
    assert add_numbers.__name__ == "add_numbers"
    assert add_numbers.__doc__ == "Add two numbers together."
    assert hasattr(add_numbers, "__wrapped__")


def test_register_callable_validation():
    registry = HandlerRegistry()
    with pytest.raises(TypeError, match="Handler must be callable"):
        registry.register("not_a_function")


def test_register_boundary_twenty_limit():
    registry = HandlerRegistry()
    for i in range(20):
        def make_h(idx):
            def h(): return idx
            h.__name__ = f"h_{idx}"
            return h
        registry.register(make_h(i))

    def h_extra(): return -1
    h_extra.__name__ = "h_extra"
    with pytest.raises(ValueError, match="Handler limit of 20 reached"):
        registry.register(h_extra)


def test_execute_registered_handler():
    registry = HandlerRegistry()
    def greet(name: str) -> str:
        return f"Hello, {name}!"
    registry.register(greet)
    res = registry.execute("greet", "World")
    assert res == "Hello, World!"
    assert registry.total_calls() == 1


def test_clear_resets_stats_and_handlers():
    registry = HandlerRegistry()
    def ping(): return "pong"
    registry.register(ping)
    registry.execute("ping")
    assert registry.total_calls() == 1
    registry.clear()
    assert registry.total_calls() == 0
    assert len(registry.handlers) == 0


def test_action_name_validation():
    with pytest.raises(ValueError, match="action_name must be a non-empty string"):
        audit_logged("")
