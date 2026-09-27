import pytest

from candidate import RefundWorkflow


@pytest.fixture
def paid():
    return []


@pytest.fixture
def flow(paid):
    return RefundWorkflow(lambda order, amount: paid.append((order, amount)))


def test_small_refund_pays_immediately(flow, paid):
    assert flow.start("t", "o1", 99) == {
        "status": "paid",
        "order_id": "o1",
        "amount": 99,
    }
    assert paid == [("o1", 99)]
    assert not flow.is_waiting("t")


def test_threshold_amount_interrupts_without_paying(flow, paid):
    out = flow.start("t", "o2", 100)
    assert out == {"status": "interrupted", "question": "Refund 100 for o2?"}
    assert paid == []
    assert flow.is_waiting("t")


def test_approve_resumes_and_pays_once(flow, paid):
    flow.start("t", "o3", 250)
    assert flow.resume("t", "approve")["status"] == "paid"
    assert paid == [("o3", 250)]
    assert not flow.is_waiting("t")
    with pytest.raises(KeyError):
        flow.resume("t", "approve")
    assert paid == [("o3", 250)]


def test_reject_resumes_without_paying(flow, paid):
    flow.start("t", "o4", 500)
    assert flow.resume("t", "reject") == {
        "status": "rejected",
        "order_id": "o4",
        "amount": 500,
    }
    assert paid == []


def test_invalid_answer_keeps_the_thread_waiting(flow, paid):
    flow.start("t", "o5", 300)
    with pytest.raises(ValueError, match="approve or reject"):
        flow.resume("t", "yes")
    assert flow.is_waiting("t")
    assert flow.resume("t", "approve")["status"] == "paid"


def test_threads_are_independent_and_single_pending(flow, paid):
    flow.start("a", "o6", 300)
    flow.start("b", "o7", 400)
    with pytest.raises(RuntimeError, match="waiting"):
        flow.start("a", "o8", 10)
    flow.resume("b", "approve")
    assert paid == [("o7", 400)]
    assert flow.is_waiting("a")


def test_unknown_thread_and_bad_amounts(flow):
    with pytest.raises(KeyError):
        flow.resume("nope", "approve")
    for amount in (0, -5):
        with pytest.raises(ValueError, match="positive"):
            flow.start("t", "o", amount)
