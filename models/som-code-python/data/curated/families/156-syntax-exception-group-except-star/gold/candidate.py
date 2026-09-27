"""Report every invalid field at once with ExceptionGroup and except*."""

from collections.abc import Iterable, Mapping


class FieldError(ValueError):
    """One field of a record failed validation."""

    def __init__(self, field: str, message: str) -> None:
        super().__init__(f"{field}: {message}")
        self.field = field


def validate_user(record: Mapping[str, object]) -> None:
    """Raise an ExceptionGroup of FieldError listing every invalid field."""
    errors: list[FieldError] = []
    name = record.get("name")
    if not isinstance(name, str) or not name.strip():
        errors.append(FieldError("name", "required"))
    age = record.get("age")
    if not isinstance(age, int) or isinstance(age, bool) or not 0 <= age <= 150:
        errors.append(FieldError("age", "must be an integer from 0 to 150"))
    email = record.get("email")
    if not isinstance(email, str) or "@" not in email:
        errors.append(FieldError("email", "must contain @"))
    if errors:
        raise ExceptionGroup("invalid user", errors)


def import_users(
    records: Iterable[Mapping[str, object]],
) -> tuple[int, dict[int, list[str]]]:
    """Return how many records passed and the failing fields by record index."""
    accepted = 0
    failures: dict[int, list[str]] = {}
    for index, record in enumerate(records):
        try:
            validate_user(record)
        except* FieldError as group:
            failures[index] = sorted(
                e.field for e in group.exceptions if isinstance(e, FieldError)
            )
        else:
            accepted += 1
    return accepted, failures
