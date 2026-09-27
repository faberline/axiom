from fastapi.testclient import TestClient

from candidate import app

client = TestClient(app)


def test_valid_request_returns_the_month_total():
    res = client.get("/sales/eu/2025/3")
    assert res.status_code == 200
    assert res.json() == {"region": "eu", "year": 2025, "month": 3, "total": 95}


def test_region_must_be_a_known_lowercase_value():
    assert client.get("/sales/mars/2025/3").status_code == 422
    assert client.get("/sales/EU/2025/3").status_code == 422


def test_month_and_year_bounds():
    assert client.get("/sales/eu/2025/12").json()["total"] == 210
    assert client.get("/sales/eu/2025/0").status_code == 422
    assert client.get("/sales/eu/2025/13").status_code == 422
    assert client.get("/sales/eu/1999/3").status_code == 422


def test_missing_year_is_404():
    res = client.get("/sales/eu/2024/3")
    assert res.status_code == 404
    assert res.json()["detail"] == "no sales for eu 2024"


def test_scale_query_parameter():
    assert client.get("/sales/us/2025/1").json()["total"] == 300
    assert client.get("/sales/us/2025/1?scale=100").json()["total"] == 3
    assert client.get("/sales/us/2025/1?scale=0").status_code == 422
