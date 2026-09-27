import pytest

from candidate import END, START, GraphError, GraphRecursionError, StateGraph


def counter_graph(limit=3):
    g = StateGraph()
    g.add_node("inc", lambda s: {"n": s["n"] + 1, "trace": [*s["trace"], "inc"]})
    g.add_node("done", lambda s: {"trace": [*s["trace"], "done"]})
    g.add_edge(START, "inc")
    g.add_conditional_edges(
        "inc",
        lambda s: "more" if s["n"] < limit else "stop",
        {"more": "inc", "stop": "done"},
    )
    g.add_edge("done", END)
    return g


def test_conditional_loop_runs_to_end():
    out = counter_graph().compile().invoke({"n": 0, "trace": []})
    assert out["n"] == 3
    assert out["trace"] == ["inc", "inc", "inc", "done"]


def test_input_state_is_not_mutated():
    state = {"n": 0, "trace": [], "keep": 1}
    out = counter_graph().compile().invoke(state)
    assert state == {"n": 0, "trace": [], "keep": 1}
    assert out["keep"] == 1


def test_recursion_limit_counts_node_steps():
    app = counter_graph(limit=10).compile()
    assert app.invoke({"n": 0, "trace": []}, recursion_limit=11)["n"] == 10
    with pytest.raises(GraphRecursionError):
        app.invoke({"n": 0, "trace": []}, recursion_limit=10)


def test_router_returning_unknown_key_raises():
    g = StateGraph()
    g.add_node("a", lambda s: {})
    g.add_edge(START, "a")
    g.add_conditional_edges("a", lambda s: "nowhere", {"x": END})
    with pytest.raises(GraphError, match="nowhere"):
        g.compile().invoke({})


def test_compile_rejects_unknown_targets():
    g = StateGraph()
    g.add_node("a", lambda s: {})
    g.add_edge(START, "a")
    g.add_conditional_edges("a", lambda s: "x", {"x": "ghost"})
    with pytest.raises(GraphError, match="ghost"):
        g.compile()
    g = StateGraph()
    g.add_node("a", lambda s: {})
    g.add_edge(START, "a")
    g.add_edge("a", "missing")
    with pytest.raises(GraphError, match="missing"):
        g.compile()


def test_compile_rejects_structural_gaps():
    g = StateGraph()
    g.add_node("a", lambda s: {})
    g.add_edge("a", END)
    with pytest.raises(GraphError, match="START"):
        g.compile()
    g = StateGraph()
    g.add_node("a", lambda s: {})
    g.add_node("b", lambda s: {})
    g.add_edge(START, "a")
    g.add_edge("a", END)
    with pytest.raises(GraphError, match="'b'"):
        g.compile()
    g = StateGraph()
    g.add_node("a", lambda s: {})
    g.add_edge(START, "a")
    g.add_edge("a", END)
    g.add_edge(END, "a")
    with pytest.raises(GraphError):
        g.compile()


def test_node_names_are_unique_and_not_reserved():
    g = StateGraph()
    g.add_node("a", lambda s: {})
    for name in ("a", START, END):
        with pytest.raises(GraphError):
            g.add_node(name, lambda s: {})
