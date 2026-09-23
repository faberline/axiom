"""Oracle test suite for 39-celery-task-shadowing."""
import pytest
import candidate


def test_eager_in_memory_execution():
    registry = candidate.SafeTaskRegistry()
    assert registry.app.conf.task_always_eager is True
    @registry.register("math.multiply")
    def multiply(a, b):
        return a * b

    res = multiply.delay(3, 4)
    assert res.get() == 12
    assert res.successful()


def test_duplicate_task_registration_raises_collision():
    registry = candidate.SafeTaskRegistry()
    @registry.register("orders.process")
    def process_v1(order_id):
        return "v1"

    with pytest.raises(candidate.TaskCollisionError):
        @registry.register("orders.process")
        def process_v2(order_id):
            return "v2"


def test_empty_and_whitespace_task_names_rejected():
    registry = candidate.SafeTaskRegistry()
    with pytest.raises(ValueError):
        registry.register("")
    with pytest.raises(ValueError):
        registry.register("   ")


def test_task_registered_under_explicit_name():
    registry = candidate.SafeTaskRegistry()
    @registry.register("billing.generate_invoice")
    def invoice_fn():
        return "ok"

    assert "billing.generate_invoice" in registry.app.tasks
    assert registry.get_task("billing.generate_invoice") is not None


def test_reset_clears_app_tasks_and_registry():
    registry = candidate.SafeTaskRegistry()
    @registry.register("reports.daily")
    def daily_report():
        return "done"

    assert "reports.daily" in registry.app.tasks
    registry.reset()
    assert "reports.daily" not in registry.app.tasks
    assert registry.list_tasks() == []
