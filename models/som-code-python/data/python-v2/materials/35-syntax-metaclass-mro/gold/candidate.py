from typing import Any


class ComponentMeta(type):
    _registry: dict[str, type] = {}

    def __new__(
        mcs, name: str, bases: tuple[type, ...], attrs: dict[str, Any]
    ) -> type:
        cls = super().__new__(mcs, name, bases, attrs)
        if name != "BaseComponent":
            component_name = attrs.get("component_name")
            if not component_name or not isinstance(component_name, str):
                raise AttributeError(f"{name} must define a non-empty string component_name")
            priority = attrs.get("priority", 0)
            if not isinstance(priority, int):
                raise TypeError("priority must be an integer")
            ComponentMeta._registry[component_name] = cls
        return cls


class BaseComponent(metaclass=ComponentMeta):
    component_name: str = "base"
    priority: int = 0

    def process(self, context: dict[str, Any]) -> dict[str, Any]:
        context.setdefault("trail", []).append("base")
        return context


class LoggingMixin:
    def process(self, context: dict[str, Any]) -> dict[str, Any]:
        context.setdefault("trail", []).append("logging")
        return super().process(context)


class ValidationMixin:
    def process(self, context: dict[str, Any]) -> dict[str, Any]:
        context.setdefault("trail", []).append("validation")
        return super().process(context)


class ComponentManager:
    def __init__(self) -> None:
        self.history: list[dict[str, Any]] = []

    def get_component(self, name: str) -> type | None:
        return ComponentMeta._registry.get(name)

    def execute_component(self, name: str, context: dict[str, Any]) -> dict[str, Any]:
        cls = self.get_component(name)
        if cls is None:
            raise KeyError(f"Component {name} not found")
        instance = cls()
        result = instance.process(context)
        self.history.append({"name": name, "trail": list(result.get("trail", []))})
        return result

    def validate_priority(self, priority: int) -> bool:
        if priority < 0 or priority > 100:
            raise ValueError("priority must be between 0 and 100 inclusive")
        return True

    def clear(self) -> None:
        ComponentMeta._registry.clear()
        self.history.clear()

    def history_count(self) -> int:
        return len(self.history)
