import pytest

from candidate import ChatGraph, MemorySaver, thread_id_of


def cfg(tid):
    return {"configurable": {"thread_id": tid}}


def echo(messages):
    return f"seen {len(messages)}"


def test_thread_resumes_from_its_checkpoint():
    graph = ChatGraph(echo, MemorySaver())
    graph.invoke("hi", cfg("t1"))
    out = graph.invoke("again", cfg("t1"))
    assert out["turns"] == 2
    assert out["messages"] == ["user: hi", "ai: seen 1", "user: again", "ai: seen 3"]


def test_threads_are_isolated():
    graph = ChatGraph(echo, MemorySaver())
    graph.invoke("a", cfg("t1"))
    graph.invoke("b", cfg("t1"))
    out = graph.invoke("x", cfg("t2"))
    assert out == {"messages": ["user: x", "ai: seen 1"], "turns": 1}
    assert graph.snapshot_count(cfg("t1")) == 2
    assert graph.snapshot_count(cfg("t2")) == 1


def test_snapshots_are_immutable_copies():
    saver = MemorySaver()
    graph = ChatGraph(echo, saver)
    out = graph.invoke("hi", cfg("t"))
    out["messages"].append("tampered")
    saver.get(cfg("t"))["messages"].append("tampered")
    saver.history(cfg("t"))[0]["messages"].append("tampered")
    assert saver.get(cfg("t"))["messages"] == ["user: hi", "ai: seen 1"]
    state = {"messages": ["m"], "turns": 5}
    saver.put(cfg("u"), state)
    state["messages"].append("later")
    assert saver.get(cfg("u"))["messages"] == ["m"]


def test_history_keeps_every_turn_in_order():
    saver = MemorySaver()
    graph = ChatGraph(echo, saver)
    for text in ("one", "two", "three"):
        graph.invoke(text, cfg("t"))
    assert [s["turns"] for s in saver.history(cfg("t"))] == [1, 2, 3]
    assert saver.history(cfg("new")) == []
    assert saver.get(cfg("new")) is None


def test_thread_id_is_required():
    graph = ChatGraph(echo, MemorySaver())
    for bad in (
        {},
        {"configurable": {}},
        {"configurable": {"thread_id": ""}},
        {"configurable": {"thread_id": 7}},
        {"configurable": None},
    ):
        with pytest.raises(ValueError, match="thread_id"):
            graph.invoke("hi", bad)
    assert thread_id_of(cfg("abc")) == "abc"
