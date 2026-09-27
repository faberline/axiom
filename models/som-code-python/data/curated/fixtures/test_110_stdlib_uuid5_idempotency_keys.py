import uuid

import pytest

from candidate import IdempotencyStore, idempotency_key


def test_key_is_a_stable_uuid5_independent_of_field_order():
    first = idempotency_key("acme", {"amount": 10, "to": "bob"})
    second = idempotency_key("acme", {"to": "bob", "amount": 10})
    assert first == second
    assert first.version == 5


def test_tenants_are_part_of_the_key():
    payload = {"amount": 10}
    assert idempotency_key("acme", payload) != idempotency_key("globex", payload)
    assert idempotency_key(" acme ", payload) == idempotency_key("acme", payload)


def test_blank_tenant_is_rejected():
    with pytest.raises(ValueError, match="tenant must not be blank"):
        idempotency_key("  ", {"amount": 1})


def test_repeat_request_replays_the_first_result():
    store = IdempotencyStore()
    calls = []

    def charge():
        calls.append(1)
        return f"charge-{len(calls)}"

    assert store.run("acme", {"amount": 5}, charge) == ("charge-1", False)
    assert store.run("acme", {"amount": 5}, charge) == ("charge-1", True)
    assert calls == [1]


def test_failed_action_is_not_remembered():
    store = IdempotencyStore()

    def boom():
        raise RuntimeError("declined")

    with pytest.raises(RuntimeError):
        store.run("acme", {"amount": 5}, boom)
    assert store.run("acme", {"amount": 5}, lambda: "ok") == ("ok", False)


def test_namespace_is_fixed():
    key = idempotency_key("acme", {})
    assert key == uuid.uuid5(uuid.UUID("6f1c2d0e-8a4b-4c3d-9e5f-1a2b3c4d5e6f"), "acme:{}")
