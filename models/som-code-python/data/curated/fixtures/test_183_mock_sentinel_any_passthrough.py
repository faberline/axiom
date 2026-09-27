"""Fixture for 183: sentinel pass-through and ANY for generated headers."""

from unittest.mock import Mock, call, sentinel

import pytest

from candidate import expected_publish, fan_out


def ids(*values: str) -> Mock:
    return Mock(side_effect=list(values))


def test_payloads_pass_through_in_order() -> None:
    broker = Mock()
    result = fan_out(
        broker, "orders", [sentinel.first, sentinel.second], ids("m1", "m2")
    )
    assert result == ["m1", "m2"]
    assert broker.mock_calls == [
        expected_publish("orders", sentinel.first),
        expected_publish("orders", sentinel.second),
    ]


def test_headers_carry_a_fresh_id_and_the_topic() -> None:
    broker = Mock()
    new_id = ids("m1", "m2", "m3")
    fan_out(broker, "orders", [sentinel.a, sentinel.b], new_id)
    assert new_id.call_count == 2
    assert broker.publish.call_args_list[1].kwargs["headers"] == {
        "message-id": "m2",
        "topic": "orders",
    }


def test_payload_object_is_not_copied() -> None:
    broker = Mock()
    payload = [1, 2, 3]
    fan_out(broker, "orders", [payload], ids("m1"))
    assert broker.publish.call_args.args[1] is payload


def test_expected_publish_ignores_headers_but_not_payload() -> None:
    expected = expected_publish("t", sentinel.x)
    assert expected == call.publish("t", sentinel.x, headers={"message-id": "any"})
    assert expected != call.publish("t", sentinel.y, headers={})
    assert expected != call.publish("u", sentinel.x, headers={})


@pytest.mark.parametrize("topic", ["", " orders", "orders "])
def test_bad_topics_publish_nothing(topic: str) -> None:
    broker = Mock()
    with pytest.raises(ValueError, match="topic"):
        fan_out(broker, topic, [sentinel.a], ids("m1"))
    broker.publish.assert_not_called()
