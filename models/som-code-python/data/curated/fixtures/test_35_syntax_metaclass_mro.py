import pytest
from candidate import (
    BaseComponent,
    ComponentManager,
    ComponentMeta,
    LoggingMixin,
    ValidationMixin,
)


def test_cooperative_multiple_inheritance_mro():
    class PipelineComponent(LoggingMixin, ValidationMixin, BaseComponent):
        component_name = "pipeline_test"

    manager = ComponentManager()
    res = manager.execute_component("pipeline_test", {})
    assert res["trail"] == ["logging", "validation", "base"]


def test_metaclass_component_name_validation():
    with pytest.raises(AttributeError, match="must define a non-empty string component_name"):
        class AnonymousComponent(BaseComponent):
            pass


def test_priority_boundary_hundred_allowed():
    manager = ComponentManager()
    assert manager.validate_priority(100) is True
    with pytest.raises(ValueError, match="priority must be between 0 and 100 inclusive"):
        manager.validate_priority(101)


def test_execute_component_success():
    class SimpleComponent(BaseComponent):
        component_name = "simple"

    manager = ComponentManager()
    res = manager.execute_component("simple", {})
    assert res["trail"] == ["base"]


def test_clear_resets_registry_and_history():
    class TempComponent(BaseComponent):
        component_name = "temp"

    manager = ComponentManager()
    manager.execute_component("temp", {})
    assert manager.history_count() == 1
    manager.clear()
    assert manager.history_count() == 0
    assert manager.get_component("temp") is None


def test_priority_type_validation():
    with pytest.raises(TypeError, match="priority must be an integer"):
        class BadPriorityComponent(BaseComponent):
            component_name = "bad_pri"
            priority = "high"
