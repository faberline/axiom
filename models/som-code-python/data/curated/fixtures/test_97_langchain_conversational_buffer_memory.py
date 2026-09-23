import pytest
from candidate import SlidingWindowMemoryBuffer


def test_system_prompt_preserved_after_window_pruning():
    buffer = SlidingWindowMemoryBuffer(max_messages=4, system_prompt="You are a helpful tutor.")
    session_id = "test-session-1"

    for i in range(10):
        buffer.add_user_message(session_id, f"Question {i}")
        buffer.add_ai_message(session_id, f"Answer {i}")

    messages = buffer.get_messages(session_id)
    assert len(messages) == 5, f"Expected 1 system + 4 dialogue messages, got {len(messages)}"
    assert messages[0]["role"] == "system"
    assert messages[0]["content"] == "You are a helpful tutor."
    assert messages[1]["content"] == "Question 8"
    assert messages[4]["content"] == "Answer 9"


def test_default_buffer_enforces_pruning():
    buffer = SlidingWindowMemoryBuffer()
    session_id = "default-session"

    for i in range(30):
        buffer.add_user_message(session_id, f"Msg {i}")

    messages = buffer.get_messages(session_id)
    assert 0 < len(messages) <= 10, f"Default buffer leaked history: {len(messages)} messages retained"


def test_exact_window_boundary():
    buffer = SlidingWindowMemoryBuffer(max_messages=3)
    session_id = "boundary-session"

    for i in range(10):
        buffer.add_user_message(session_id, f"Step {i}")

    messages = buffer.get_messages(session_id)
    assert len(messages) == 3, f"Expected exact boundary of 3, got {len(messages)}"
    assert [m["content"] for m in messages] == ["Step 7", "Step 8", "Step 9"]


def test_session_capacity_eviction_lru():
    buffer = SlidingWindowMemoryBuffer(max_sessions=3)

    buffer.add_user_message("sess-1", "A")
    buffer.add_user_message("sess-2", "B")
    buffer.add_user_message("sess-3", "C")
    assert buffer.get_session_count() == 3

    # Access sess-1 so sess-2 becomes the oldest/least recently used
    _ = buffer.get_messages("sess-1")

    # Add sess-4, should evict sess-2
    buffer.add_user_message("sess-4", "D")
    assert buffer.get_session_count() == 3, f"Expected 3 sessions capped, got {buffer.get_session_count()}"
    assert buffer.get_messages("sess-2") == [], "LRU session sess-2 was not evicted"
    assert len(buffer.get_messages("sess-1")) > 0
    assert len(buffer.get_messages("sess-4")) > 0


def test_prune_history_mutates_underlying_storage():
    buffer = SlidingWindowMemoryBuffer(max_messages=2)
    session_id = "storage-test"

    buffer.add_user_message(session_id, "One")
    buffer.add_user_message(session_id, "Two")
    buffer.add_user_message(session_id, "Three")

    messages = buffer.get_messages(session_id)
    assert len(messages) == 2, f"Expected 2 messages in underlying storage, got {len(messages)}"
    assert [m["content"] for m in messages] == ["Two", "Three"]
