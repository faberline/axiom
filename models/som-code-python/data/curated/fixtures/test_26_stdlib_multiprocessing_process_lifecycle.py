import multiprocessing as mp
import threading
import time
import pytest
from candidate import ResilientWorkerPool


def test_pool_lifecycle_and_results():
    pool = ResilientWorkerPool(num_workers=2)
    pool.start()
    try:
        pool.submit(10)
        pool.submit(20)
        results = pool.collect_results(expected_count=2, timeout=2.0)
        assert len(results) == 2
        vals = sorted([r["result"] for r in results])
        assert vals == [20, 40]
    finally:
        pool.shutdown(timeout=1.0)
    for p in pool.workers:
        assert not p.is_alive(), "Worker process must be stopped"
        assert p.exitcode is not None, "Worker exitcode must be reaped"


def test_zombie_reaping_on_forced_termination(monkeypatch):
    pool = ResilientWorkerPool(num_workers=1)
    pool.start()
    worker = pool.workers[0]
    call_sequence = []

    orig_terminate = worker.terminate
    orig_join = worker.join

    def spy_terminate():
        call_sequence.append("terminate")
        return orig_terminate()

    def spy_join(timeout=None):
        call_sequence.append("join" if timeout is None else f"join_timeout_{timeout}")
        return orig_join(timeout)

    monkeypatch.setattr(worker, "terminate", spy_terminate)
    monkeypatch.setattr(worker, "join", spy_join)
    monkeypatch.setattr(worker, "is_alive", lambda: "terminate" not in call_sequence)

    pool.shutdown(timeout=0.1)

    assert "terminate" in call_sequence, "p.terminate() was not called"
    term_idx = call_sequence.index("terminate")
    reap_joins = [c for c in call_sequence[term_idx + 1:] if c == "join"]
    assert len(reap_joins) > 0, "p.join() was not called after p.terminate() to reap zombie"


def test_collect_results_times_out_when_no_items():
    pool = ResilientWorkerPool(num_workers=1)
    pool.start()
    err = None

    def runner():
        nonlocal err
        try:
            pool.collect_results(expected_count=1, timeout=0.2)
        except Exception as exc:
            err = exc

    t = threading.Thread(target=runner, daemon=True)
    t.start()
    t.join(timeout=0.4)
    pool.shutdown(timeout=0.5)

    assert not t.is_alive(), "collect_results blocked indefinitely on empty queue"
    assert isinstance(err, TimeoutError), f"Expected TimeoutError, got {err}"


def test_submit_rejects_when_stopped():
    pool = ResilientWorkerPool(num_workers=1)
    with pytest.raises(RuntimeError):
        pool.submit(42)


def test_double_start_idempotent():
    pool = ResilientWorkerPool(num_workers=2)
    pool.start()
    try:
        pool.start()
        assert len(pool.workers) == 2, f"Expected 2 workers, got {len(pool.workers)}"
    finally:
        pool.shutdown(timeout=0.5)


def test_graceful_shutdown_sentinels(monkeypatch):
    pool = ResilientWorkerPool(num_workers=1)
    pool.start()
    sentinels_queued = []
    orig_put = pool.task_queue.put

    def track_put(item):
        sentinels_queued.append(item)
        return orig_put(item)

    monkeypatch.setattr(pool.task_queue, "put", track_put)
    pool.shutdown(timeout=1.0)
    assert "STOP_SENTINEL" in sentinels_queued
