import csv
import io

from candidate import RowError, import_contacts


def rows(text):
    return csv.DictReader(io.StringIO(text))


def test_valid_rows_are_coerced():
    contacts, errors = import_contacts(
        rows("name,age,newsletter\nAda,36,yes\nBob,150,false\n")
    )
    assert errors == []
    assert contacts == [
        {"name": "Ada", "age": 36, "newsletter": True},
        {"name": "Bob", "age": 150, "newsletter": False},
    ]


def test_errors_carry_file_line_and_field():
    contacts, errors = import_contacts(
        rows("name,age,newsletter\nAda,36,yes\n,40,no\nCy,abc,no\nDee,20,no\n")
    )
    assert [c["name"] for c in contacts] == ["Ada", "Dee"]
    assert [(e.line, e.field) for e in errors] == [(3, "name"), (4, "age")]
    assert all(isinstance(e, RowError) and e.message for e in errors)


def test_every_failing_field_in_a_row_is_reported():
    _, errors = import_contacts(rows("name,age,newsletter\n,200,maybe\n"))
    assert [(e.line, e.field) for e in errors] == [
        (2, "name"),
        (2, "age"),
        (2, "newsletter"),
    ]


def test_missing_column_is_reported_per_row():
    contacts, errors = import_contacts(rows("name,age\nAda,36\nBob,40\n"))
    assert contacts == []
    assert [(e.line, e.field) for e in errors] == [(2, "newsletter"), (3, "newsletter")]
