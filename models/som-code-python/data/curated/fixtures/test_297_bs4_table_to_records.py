import pytest

from candidate import TableError, table_records

PAGE = """
<table id="other"><tr><th>x</th></tr><tr><td>nope</td></tr></table>
<table id="people">
  <thead><tr><th>Full Name</th><th>Age</th><th>City</th></tr></thead>
  <tbody>
    <tr><td>Ada <b>Lovelace</b></td><td>36</td><td>London</td></tr>
    <tr><td>Alan Turing</td><td></td></tr>
    <tr></tr>
    <tr><td>Grace</td><td>85</td><td>  Arlington </td></tr>
  </tbody>
</table>
"""


def test_records_use_normalised_header_names():
    records = table_records(PAGE, "people")
    assert records[0] == {"full_name": "Ada Lovelace", "age": "36", "city": "London"}
    assert records[2] == {"full_name": "Grace", "age": "85", "city": "Arlington"}


def test_short_rows_and_empty_cells_become_none():
    assert table_records(PAGE, "people")[1] == {
        "full_name": "Alan Turing",
        "age": None,
        "city": None,
    }


def test_empty_rows_are_skipped_and_table_is_selected_by_id():
    assert len(table_records(PAGE, "people")) == 3
    assert table_records(PAGE, "other") == [{"x": "nope"}]


def test_header_without_thead():
    html = "<table id='t'><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>2</td></tr></table>"
    assert table_records(html, "t") == [{"a": "1", "b": "2"}]


def test_errors():
    with pytest.raises(TableError):
        table_records(PAGE, "missing")
    with pytest.raises(TableError):
        table_records("<table id='t'><tr><th>a</th><th>A</th></tr></table>", "t")
    with pytest.raises(TableError):
        table_records(
            "<table id='t'><tr><th>a</th></tr><tr><td>1</td><td>2</td></tr></table>",
            "t",
        )
    with pytest.raises(TableError):
        table_records("<table id='t'></table>", "t")
    assert issubclass(TableError, ValueError)
