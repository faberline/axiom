import pytest
from fastapi.testclient import TestClient
from starlette.websockets import WebSocketDisconnect

import candidate
from candidate import app

client = TestClient(app)


@pytest.fixture(autouse=True)
def fresh():
    candidate.reset_db()


def test_valid_token_echoes_uppercase_with_user():
    with client.websocket_connect("/ws?token=tok-ada") as ws:
        assert ws.receive_json() == {"hello": "ada"}
        ws.send_text("ping")
        assert ws.receive_json() == {"user": "ada", "echo": "PING"}
        ws.send_text("bye")
        with pytest.raises(WebSocketDisconnect) as info:
            ws.receive_json()
        assert info.value.code == 1000
    assert candidate.ONLINE == set()


def test_missing_token_is_closed_with_policy_violation():
    with pytest.raises(WebSocketDisconnect) as info:
        with client.websocket_connect("/ws") as ws:
            ws.receive_json()
    assert info.value.code == 1008


def test_unknown_token_is_closed_with_policy_violation():
    with pytest.raises(WebSocketDisconnect) as info:
        with client.websocket_connect("/ws?token=forged") as ws:
            ws.receive_json()
    assert info.value.code == 1008
    assert candidate.ONLINE == set()


def test_online_set_tracks_connected_users():
    with client.websocket_connect("/ws?token=tok-bob") as ws:
        ws.receive_json()
        ws.send_text("x")
        ws.receive_json()
        assert candidate.ONLINE == {"bob"}
    assert candidate.ONLINE == set()
