"""Validate imported CSV rows one by one and report every failing field."""

from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from typing import Annotated, TypedDict

from pydantic import Field, TypeAdapter, ValidationError


class Contact(TypedDict):
    """One imported contact after validation and coercion."""

    name: Annotated[str, Field(min_length=1)]
    age: Annotated[int, Field(ge=0, le=150)]
    newsletter: bool


_CONTACT: TypeAdapter[Contact] = TypeAdapter(Contact)


@dataclass(frozen=True)
class RowError:
    """A validation problem at a file line and field."""

    line: int
    field: str
    message: str


def import_contacts(
    rows: Iterable[Mapping[str, str]],
) -> tuple[list[Contact], list[RowError]]:
    """Return the valid contacts and one error per failing field.

    Line numbers count the CSV header as line 1, so the first row is line 2.
    """
    contacts: list[Contact] = []
    errors: list[RowError] = []
    for line, row in enumerate(rows, start=1):
        try:
            contacts.append(_CONTACT.validate_python(dict(row)))
        except ValidationError as exc:
            for error in exc.errors():
                field = ".".join(str(part) for part in error["loc"])
                errors.append(RowError(line, field, error["msg"]))
    return contacts, errors
