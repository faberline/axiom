"""A context manager that swallows only the expected errors, up to a budget."""

from types import TracebackType
from typing import Self


class Tolerate:
    """Suppress and record the listed exception types, at most budget times."""

    def __init__(self, *kinds: type[Exception], budget: int = 3) -> None:
        if not kinds:
            raise ValueError("name at least one exception type")
        if budget < 1:
            raise ValueError("budget must be at least 1")
        self.kinds = kinds
        self.budget = budget
        self.errors: list[Exception] = []

    def __enter__(self) -> Self:
        return type(self)(*self.kinds, budget=self.budget)

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> bool:
        if not isinstance(exc, self.kinds):
            return False
        if len(self.errors) >= self.budget:
            return False
        self.errors.append(exc)
        return True
