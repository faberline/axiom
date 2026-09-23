import pytest
from candidate import TestStateContainer, managed_state_fixture

def test_default_fixture_state_isolation():
    with managed_state_fixture() as c1:
        assert c1.is_pristine is True
        c1.set("session_user", "admin")
        assert c1.get("session_user") == "admin"
    with managed_state_fixture() as c2:
        assert c2.is_pristine is True
        assert c2.get("session_user") is None

def test_shared_instance_cleaned_up_on_exit():
    shared = TestStateContainer()
    with managed_state_fixture(shared) as c:
        c.set("token", "xyz")
        assert c.is_pristine is False
    assert shared.is_pristine is True
    assert shared.get("token") is None

def test_container_clear_resets_store_and_history():
    c = TestStateContainer()
    c.set("k1", "v1")
    assert c.is_pristine is False
    c.clear()
    assert c.is_pristine is True
    assert c.get("k1") is None

def test_cleanup_guaranteed_on_exception():
    shared = TestStateContainer()
    with pytest.raises(RuntimeError, match="simulated failure"):
        with managed_state_fixture(shared) as c:
            c.set("k1", "v1")
            raise RuntimeError("simulated failure")
    assert shared.is_pristine is True
