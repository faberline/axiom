import pytest
from unittest.mock import MagicMock
import candidate
from candidate import ResilientChatSession


class MockRateLimitError(Exception):
    pass


class MockContextLengthExceeded(Exception):
    pass


class MockMessageObj:
    def __init__(self, content: str):
        self.content = content


class MockChoiceObj:
    def __init__(self, content: str):
        self.message = MockMessageObj(content)


class MockResponseObj:
    def __init__(self, content: str):
        self.choices = [MockChoiceObj(content)]


def test_prune_history_preserves_system_prompt():
    mock_client = MagicMock()
    mock_client.chat.completions.create.side_effect = [
        MockContextLengthExceeded("context_length_exceeded: maximum tokens reached"),
        MockResponseObj("compaction success"),
    ]

    session = ResilientChatSession(mock_client)
    session.add_message("system", "You are a helpful assistant.")
    session.add_message("user", "Hello turn 1")
    session.add_message("assistant", "Response 1")
    session.add_message("user", "Hello turn 2")
    session.add_message("assistant", "Response 2")

    ans = session.send_message("New prompt")
    assert ans == "compaction success"
    assert len(session.messages) >= 1
    assert session.messages[0]["role"] == "system", "System prompt was purged during context compaction!"
    assert session.messages[0]["content"] == "You are a helpful assistant."


def test_retry_boundary_allows_configured_attempts():
    mock_client = MagicMock()
    mock_client.chat.completions.create.side_effect = [
        MockRateLimitError("429 rate limit exceeded"),
        MockRateLimitError("429 rate limit exceeded"),
        MockRateLimitError("429 rate limit exceeded"),
        MockResponseObj("recovered after 3 retries"),
    ]

    session = ResilientChatSession(mock_client, max_retries=3, base_backoff=0.001)
    ans = session.send_message("retry test")
    assert ans == "recovered after 3 retries"
    assert mock_client.chat.completions.create.call_count == 4


def test_default_max_retries_is_three():
    mock_client = MagicMock()
    session = ResilientChatSession(mock_client)
    assert session.max_retries == 3, f"Expected default max_retries=3, got {session.max_retries}"


def test_prune_history_short_conversation_validation():
    mock_client = MagicMock()
    session = ResilientChatSession(mock_client)
    session.add_message("user", "hi")
    session.add_message("assistant", "hello")

    pruned = session.prune_history_for_context()
    assert pruned is False, "Expected pruning to return False on history of length 2"
    assert len(session.messages) == 2


def test_send_message_calls_chat_completions():
    mock_client = MagicMock()
    mock_client.chat.completions.create.return_value = MockResponseObj("standard output")

    session = ResilientChatSession(mock_client)
    ans = session.send_message("hello")
    assert ans == "standard output"
    mock_client.chat.completions.create.assert_called_once()
