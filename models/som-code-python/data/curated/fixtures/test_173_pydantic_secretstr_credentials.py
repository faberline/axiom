import pytest
from pydantic import ValidationError

from candidate import DatabaseConfig, log_fields

SECRET = "p@ss/w:rd"


def make(**overrides):
    fields = {"host": "db", "user": "app", "password": SECRET, "database": "shop"}
    fields.update(overrides)
    return DatabaseConfig(**fields)


def test_repr_and_str_hide_the_password():
    config = make()
    assert SECRET not in repr(config)
    assert SECRET not in str(config)


def test_log_fields_mask_the_password():
    fields = log_fields(make())
    assert fields == {
        "host": "db",
        "port": 5432,
        "user": "app",
        "password": "**********",
        "database": "shop",
    }


def test_url_percent_encodes_the_password():
    assert make().url() == "postgresql://app:p%40ss%2Fw%3Ard@db:5432/shop"


def test_redacted_url():
    assert make(port=6543).redacted_url() == "postgresql://app:***@db:6543/shop"


@pytest.mark.parametrize("port", [0, 65536])
def test_port_out_of_range(port):
    with pytest.raises(ValidationError):
        make(port=port)


def test_highest_port_is_allowed():
    assert make(port=65535).port == 65535


def test_blank_host_is_rejected():
    with pytest.raises(ValidationError):
        make(host="")
