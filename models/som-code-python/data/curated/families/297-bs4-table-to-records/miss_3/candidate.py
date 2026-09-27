"""Turn an HTML table into a list of dict records with BeautifulSoup."""

from __future__ import annotations

from bs4 import BeautifulSoup, Tag


class TableError(ValueError):
    """The requested table is missing or malformed."""


def _cells(row: Tag) -> list[str]:
    return [cell.get_text(" ", strip=True) for cell in row.find_all(["th", "td"])]


def _header(table: Tag) -> list[str]:
    head = table.find("thead")
    row = head.find("tr") if isinstance(head, Tag) else table.find("tr")
    if not isinstance(row, Tag):
        raise TableError("table has no header row")
    names = [name.replace(" ", "_") for name in _cells(row)]
    if len(set(names)) != len(names):
        raise TableError(f"duplicate column names: {names}")
    return names


def table_records(html: str, table_id: str) -> list[dict[str, str | None]]:
    """Return one dict per body row of the table with the given id."""
    soup = BeautifulSoup(html, "html.parser")
    table = soup.find("table", id=table_id)
    if not isinstance(table, Tag):
        raise TableError(f"no table with id {table_id!r}")
    names = _header(table)
    rows = table.find_all("tr")[1:]
    records: list[dict[str, str | None]] = []
    for row in rows:
        values = _cells(row)
        if not values:
            continue
        if len(values) > len(names):
            raise TableError(f"row has {len(values)} cells for {len(names)} columns")
        padded: list[str | None] = [value or None for value in values]
        padded += [None] * (len(names) - len(values))
        records.append(dict(zip(names, padded, strict=True)))
    return records
