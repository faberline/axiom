"""Safe Celery task registry preventing task shadowing and duplicate registration."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from celery import Celery, Task


class TaskCollisionError(ValueError):
    """Raised when attempting to register a task whose name is already taken."""


class SafeTaskRegistry:
    """Task registry preventing silent task shadowing in Celery applications."""

    def __init__(self, app_name: str = "som_tasks", eager: bool = True) -> None:
        self.app = Celery(app_name)
        self.app.conf.update(
            task_always_eager=eager,
            task_eager_propagates=True,
            broker_url="memory://",
            result_backend="cache+memory://",
        )
        self._registered_names: set[str] = set()

    def register(
        self,
        name: str,
        bind: bool = False,
        max_retries: int = 3,
    ) -> Callable[[Callable[..., Any]], Task[Any, Any]]:
        """Register a Celery task with explicit unique naming to prevent shadowing."""
        if not name or not isinstance(name, str) or not name.strip():
            raise ValueError("Task name must be a non-empty string")
        clean_name = name.strip()
        if clean_name in self._registered_names or clean_name in self.app.tasks:
            raise TaskCollisionError(
                f"Task '{clean_name}' is already registered; shadowing is prohibited"
            )

        def decorator(fn: Callable[..., Any]) -> Task[Any, Any]:
            # celery-types overloads only literal bind values, not a runtime bool.
            task: Task[Any, Any] = self.app.task(  # type: ignore[call-overload]
                name=clean_name,
                bind=bind,
                max_retries=max_retries,
            )(fn)
            self._registered_names.add(clean_name)
            return task

        return decorator

    def get_task(self, name: str) -> Task[Any, Any] | None:
        """Retrieve registered task by canonical name."""
        return self.app.tasks.get(name.strip() if isinstance(name, str) else "")

    def list_tasks(self) -> list[str]:
        """List all custom tasks registered through this registry."""
        return sorted(list(self._registered_names))

    def reset(self) -> None:
        """Unregister all custom tasks and clear registry state."""
        self._registered_names.clear()
