"""Register plugin components through a metaclass and chain them by MRO."""

from typing import TYPE_CHECKING, Any, ClassVar, cast


class ComponentMeta(type):
    """Validate and register every concrete component class at creation."""

    registry: ClassVar[dict[str, type["BaseComponent"]]] = {}

    def __new__(mcs, name: str, bases: tuple[type, ...], attrs: dict[str, Any]) -> type:
        cls = super().__new__(mcs, name, bases, attrs)
        if name != "BaseComponent":
            component_name = attrs.get("component_name")
            if not component_name or not isinstance(component_name, str):
                raise AttributeError(
                    f"{name} must define a non-empty string component_name"
                )
            priority = attrs.get("priority", 0)
            if not isinstance(priority, int):
                raise TypeError("priority must be an integer")
            ComponentMeta.registry[component_name] = cast("type[BaseComponent]", cls)
        return cls


class BaseComponent(metaclass=ComponentMeta):
    """Root of the component hierarchy; ends every process chain."""

    component_name: str = "base"
    priority: int = 0

    def process(self, context: dict[str, Any]) -> dict[str, Any]:
        """Append base to the trail and return the context."""
        context.setdefault("trail", []).append("base")
        return context


# Type checkers see the mixins' eventual base, so super().process() resolves.
if TYPE_CHECKING:
    _MixinBase = BaseComponent
else:
    _MixinBase = object


class LoggingMixin(_MixinBase):
    """Record logging in the trail, then defer to the next class."""

    def process(self, context: dict[str, Any]) -> dict[str, Any]:
        context.setdefault("trail", []).append("logging")
        return BaseComponent.process(self, context)


class ValidationMixin(_MixinBase):
    """Record validation in the trail, then defer to the next class."""

    def process(self, context: dict[str, Any]) -> dict[str, Any]:
        context.setdefault("trail", []).append("validation")
        return super().process(context)


class ComponentManager:
    """Look up, run, and track registered components."""

    def __init__(self) -> None:
        self.history: list[dict[str, Any]] = []

    def get_component(self, name: str) -> type[BaseComponent] | None:
        """Return the component class registered under name, if any."""
        return ComponentMeta.registry.get(name)

    def execute_component(self, name: str, context: dict[str, Any]) -> dict[str, Any]:
        """Instantiate and run a component, recording its trail."""
        cls = self.get_component(name)
        if cls is None:
            raise KeyError(f"Component {name} not found")
        instance = cls()
        result = instance.process(context)
        self.history.append({"name": name, "trail": list(result.get("trail", []))})
        return result

    def validate_priority(self, priority: int) -> bool:
        """Accept a priority from 0 to 100 inclusive."""
        if priority < 0 or priority > 100:
            raise ValueError("priority must be between 0 and 100 inclusive")
        return True

    def clear(self) -> None:
        """Empty the registry and the execution history."""
        ComponentMeta.registry.clear()
        self.history.clear()

    def history_count(self) -> int:
        """Return how many executions were recorded."""
        return len(self.history)
