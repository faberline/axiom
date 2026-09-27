import csv
import io

from fastapi.testclient import TestClient

import candidate
from candidate import app, csv_rows

client = TestClient(app)


def test_export_headers_and_attachment_name():
    r = client.get("/orders.csv")
    assert r.status_code == 200
    assert r.headers["content-type"].startswith("text/csv")
    assert r.headers["content-disposition"] == 'attachment; filename="orders.csv"'


def test_body_is_valid_csv_with_quoting():
    r = client.get("/orders.csv")
    rows = list(csv.reader(io.StringIO(r.text)))
    assert rows[0] == ["id", "customer", "total"]
    assert rows[1] == ["1", "Ada, Countess", "12.50"]
    assert rows[2] == ["2", 'Bob "the builder"', "3.00"]
    assert len(rows) == 1 + len(candidate.ORDERS)
    assert "\r\n" in r.text


def test_status_filter():
    r = client.get("/orders.csv", params={"status": "open"})
    rows = list(csv.reader(io.StringIO(r.text)))
    assert [row[0] for row in rows[1:]] == ["1", "3"]


def test_rows_are_generated_lazily():
    gen = csv_rows(iter([]))
    assert next(gen) == "id,customer,total\r\n"
    pulled = []

    def source():
        for order in candidate.ORDERS:
            pulled.append(order["id"])
            yield order

    lazy = csv_rows(source())
    next(lazy)
    assert pulled == []
    next(lazy)
    assert pulled == [1]
