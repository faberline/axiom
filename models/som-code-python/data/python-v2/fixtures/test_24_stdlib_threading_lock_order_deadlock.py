import threading
import time
import pytest
from candidate import Account, SafeTransferManager


def test_safe_transfer_success():
    mgr = SafeTransferManager()
    acc1 = Account(1, 100)
    acc2 = Account(2, 50)
    res = mgr.transfer(acc1, acc2, 30)
    assert res is True
    assert acc1.balance == 70
    assert acc2.balance == 80


def test_concurrent_transfers_no_deadlock():
    mgr = SafeTransferManager()
    acc1 = Account(1, 1000)
    acc2 = Account(2, 1000)

    class InterceptedLock:
        def __init__(self, real_lock, sleep_s=0.005):
            self.real_lock = real_lock
            self.sleep_s = sleep_s

        def __enter__(self):
            res = self.real_lock.__enter__()
            time.sleep(self.sleep_s)
            return res

        def __exit__(self, *args):
            return self.real_lock.__exit__(*args)

    acc1.lock = InterceptedLock(acc1.lock)
    acc2.lock = InterceptedLock(acc2.lock)

    start_gate = threading.Barrier(2)

    def worker1():
        start_gate.wait()
        mgr.transfer(acc1, acc2, 10)

    def worker2():
        start_gate.wait()
        mgr.transfer(acc2, acc1, 10)

    t1 = threading.Thread(target=worker1, daemon=True)
    t2 = threading.Thread(target=worker2, daemon=True)

    t1.start()
    t2.start()

    deadline = time.monotonic() + 0.25
    t1.join(timeout=max(0.01, deadline - time.monotonic()))
    t2.join(timeout=max(0.01, deadline - time.monotonic()))

    assert not (t1.is_alive() or t2.is_alive()), (
        "Concurrent bidirectional transfers deadlocked due to defective lock acquisition order"
    )
    assert acc1.balance == 1000
    assert acc2.balance == 1000


def test_lock_acquisition_hierarchy():
    mgr = SafeTransferManager()
    acc1 = Account(1, 100)
    acc2 = Account(2, 100)
    order = []

    class MonitoredLock:
        def __init__(self, acc_id, real_lock):
            self.acc_id = acc_id
            self.real_lock = real_lock

        def __enter__(self):
            order.append(self.acc_id)
            return self.real_lock.__enter__()

        def __exit__(self, *args):
            return self.real_lock.__exit__(*args)

    acc1.lock = MonitoredLock(1, acc1.lock)
    acc2.lock = MonitoredLock(2, acc2.lock)

    order.clear()
    mgr.transfer(acc2, acc1, 10)
    assert order == [1, 2], f"Expected lock acquisition in ascending account_id order [1, 2], got {order}"

    order.clear()
    mgr.transfer(acc1, acc2, 10)
    assert order == [1, 2], f"Expected lock acquisition in ascending account_id order [1, 2], got {order}"


def test_self_transfer_no_deadlock():
    mgr = SafeTransferManager()
    acc = Account(1, 100)

    def worker():
        mgr.transfer(acc, acc, 20)

    t = threading.Thread(target=worker, daemon=True)
    t.start()
    t.join(timeout=0.3)
    assert not t.is_alive(), "Self-transfer deadlocked acquiring same lock twice"
    assert acc.balance == 100


def test_insufficient_funds_rejected():
    mgr = SafeTransferManager()
    acc1 = Account(1, 50)
    acc2 = Account(2, 50)
    res = mgr.transfer(acc1, acc2, 100)
    assert res is False
    assert acc1.balance == 50
    assert acc2.balance == 50


def test_rollback_on_credit_failure():
    mgr = SafeTransferManager()
    acc1 = Account(1, 100)

    class FailingAccount(Account):
        def __init__(self, account_id: int, balance: int):
            self.account_id = account_id
            self._bal = balance
            self.lock = threading.Lock()
            self.fail_credit = False

        @property
        def balance(self):
            return self._bal

        @balance.setter
        def balance(self, val):
            if self.fail_credit:
                raise RuntimeError("Credit simulation failure")
            self._bal = val

    failing_acc = FailingAccount(2, 50)
    failing_acc.fail_credit = True

    with pytest.raises(RuntimeError):
        mgr.transfer(acc1, failing_acc, 30)

    assert acc1.balance == 100, f"Expected source balance restored to 100, got {acc1.balance}"
