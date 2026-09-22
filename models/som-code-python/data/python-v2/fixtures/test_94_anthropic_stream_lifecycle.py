import pytest
from unittest.mock import MagicMock
import candidate
from candidate import AnthropicStreamHandler, StreamSummary


class MockFinalMessage:
    def __init__(self, stop_reason: str = "end_turn"):
        self.stop_reason = stop_reason


class MockStreamContext:
    def __init__(self, chunks: list[str], stop_reason: str = "end_turn"):
        self.chunks = chunks
        self.stop_reason = stop_reason
        self.is_consumed = False
        self.closed = False

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.closed = True

    @property
    def text_stream(self):
        for c in self.chunks:
            yield c
        self.is_consumed = True

    def get_final_message(self):
        if not self.is_consumed:
            raise RuntimeError("Stream is not finished yet: cannot retrieve final message before consumption")
        return MockFinalMessage(self.stop_reason)


def test_stream_context_cleanup_on_completion():
    mock_client = MagicMock()
    stream_ctx = MockStreamContext(["Part 1", " Part 2"], stop_reason="end_turn")
    mock_client.messages.stream.return_value = stream_ctx

    handler = AnthropicStreamHandler(mock_client)
    summary = handler.collect_stream(
        messages=[{"role": "user", "content": "Write a sentence"}]
    )

    assert summary.text == "Part 1 Part 2"
    assert summary.stop_reason == "end_turn"
    assert summary.is_truncated is False
    assert stream_ctx.closed is True, "Stream context manager __exit__ was not invoked!"


def test_truncation_detection_on_max_tokens():
    mock_client = MagicMock()
    stream_ctx = MockStreamContext(["Incomplete output..."], stop_reason="max_tokens")
    mock_client.messages.stream.return_value = stream_ctx

    handler = AnthropicStreamHandler(mock_client)
    summary = handler.collect_stream(
        messages=[{"role": "user", "content": "Write an essay"}]
    )

    assert summary.stop_reason == "max_tokens"
    assert summary.is_truncated is True, "Expected is_truncated=True when stop_reason is max_tokens"


def test_empty_messages_validation():
    mock_client = MagicMock()
    handler = AnthropicStreamHandler(mock_client)
    with pytest.raises(ValueError, match="messages must not be empty"):
        handler.collect_stream(messages=[])

    with pytest.raises(ValueError, match="messages must not be empty"):
        list(handler.stream_text_chunks(messages=[]))


def test_default_max_tokens_parameter():
    mock_client = MagicMock()
    stream_ctx = MockStreamContext(["ok"])
    mock_client.messages.stream.return_value = stream_ctx

    handler = AnthropicStreamHandler(mock_client)
    handler.collect_stream(messages=[{"role": "user", "content": "hi"}])

    mock_client.messages.stream.assert_called_once()
    _, kwargs = mock_client.messages.stream.call_args
    assert kwargs.get("max_tokens") == 1024, f"Expected default max_tokens=1024, got {kwargs.get('max_tokens')}"


def test_final_message_retrieval_sequence():
    mock_client = MagicMock()
    stream_ctx = MockStreamContext(["chunk1", "chunk2"])
    mock_client.messages.stream.return_value = stream_ctx

    handler = AnthropicStreamHandler(mock_client)
    summary = handler.collect_stream(messages=[{"role": "user", "content": "test"}])
    assert summary.chunk_count == 2


def test_stream_text_chunks_generator():
    mock_client = MagicMock()
    stream_ctx = MockStreamContext(["A", "B", "C"])
    mock_client.messages.stream.return_value = stream_ctx

    handler = AnthropicStreamHandler(mock_client)
    chunks = list(handler.stream_text_chunks(messages=[{"role": "user", "content": "abc"}]))
    assert chunks == ["A", "B", "C"]
    assert stream_ctx.closed is True
