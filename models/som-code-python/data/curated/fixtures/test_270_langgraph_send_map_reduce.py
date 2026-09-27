import pytest

from candidate import MAX_FANOUT, MapReduceGraph, Send, fan_out


def upper(doc):
    return doc.upper()


def test_one_send_per_document_with_private_payload():
    sends = fan_out({"documents": ["a", "b"], "secret": "x"})
    assert sends == [
        Send("summarize", {"doc": "a", "index": 0}),
        Send("summarize", {"doc": "b", "index": 1}),
    ]
    graph = MapReduceGraph(upper)
    assert graph.payload_keys({"documents": ["a"], "secret": "x"}) == [{"doc", "index"}]


def test_summaries_are_reduced_in_document_order():
    out = MapReduceGraph(upper).invoke({"documents": ["b", "a", "c"], "user": "u"})
    assert out["summaries"] == ["B", "A", "C"]
    assert out["final"] == "B\nA\nC"
    assert out["user"] == "u"


def test_duplicate_documents_are_all_kept():
    out = MapReduceGraph(upper).invoke({"documents": ["x", "x"]})
    assert out["summaries"] == ["X", "X"]


def test_empty_input_skips_workers():
    calls = []
    graph = MapReduceGraph(lambda d: calls.append(d) or d)
    out = graph.invoke({"documents": []})
    assert calls == []
    assert out["summaries"] == []
    assert out["final"] == "(no documents)"


def test_input_state_is_not_mutated():
    state = {"documents": ["a"]}
    MapReduceGraph(upper).invoke(state)
    assert state == {"documents": ["a"]}


def test_fanout_limit_is_inclusive():
    graph = MapReduceGraph(upper)
    docs = [str(i) for i in range(MAX_FANOUT)]
    assert graph.worker_count({"documents": docs}) == MAX_FANOUT
    with pytest.raises(ValueError, match="at most"):
        graph.invoke({"documents": [*docs, "extra"]})
