import pytest

from candidate import (
    Document,
    InMemoryVectorStore,
    ScoreThresholdRetriever,
    cosine,
)

VECTORS = {
    "q": [1.0, 0.0],
    "exact": [1.0, 0.0],
    "close": [0.8, 0.6],
    "edge": [0.6, 0.8],
    "far": [0.0, 1.0],
    "zero": [0.0, 0.0],
}


def embed(text):
    return VECTORS[text.split("#")[0]]


def store_with(*docs):
    store = InMemoryVectorStore(embed)
    store.add_documents(list(docs))
    return store


def doc(text, doc_id, **meta):
    return Document(text, {"id": doc_id, **meta})


def contents(docs):
    return [d.page_content for d in docs]


def test_results_are_ranked_and_threshold_is_inclusive():
    store = store_with(doc("far", 1), doc("edge", 2), doc("exact", 3), doc("close", 4))
    retriever = ScoreThresholdRetriever(store, k=10, score_threshold=0.6)
    assert contents(retriever.invoke("q")) == ["exact", "close", "edge"]


def test_duplicate_chunks_of_one_source_are_returned_once():
    store = store_with(doc("exact#a", "s1"), doc("exact#b", "s1"), doc("close", "s2"))
    retriever = ScoreThresholdRetriever(store, k=5)
    assert contents(retriever.invoke("q")) == ["exact#a", "close"]


def test_filter_is_applied_before_k():
    store = store_with(
        doc("exact", 1, lang="fr"),
        doc("close", 2, lang="fr"),
        doc("edge", 3, lang="en"),
        doc("far", 4, lang="en"),
    )
    retriever = ScoreThresholdRetriever(store, k=2, metadata_filter={"lang": "en"})
    assert contents(retriever.invoke("q")) == ["edge", "far"]


def test_k_limits_the_result_count():
    store = store_with(doc("exact", 1), doc("close", 2), doc("edge", 3))
    assert contents(ScoreThresholdRetriever(store, k=2).invoke("q")) == [
        "exact",
        "close",
    ]


def test_zero_vectors_score_zero():
    assert cosine([0.0, 0.0], [1.0, 0.0]) == 0.0
    store = store_with(doc("zero", 1), doc("close", 2))
    assert contents(ScoreThresholdRetriever(store).invoke("q")) == ["close", "zero"]


def test_invalid_configuration_is_rejected():
    store = store_with()
    for kwargs in ({"k": 0}, {"score_threshold": -0.1}, {"score_threshold": 1.5}):
        with pytest.raises(ValueError):
            ScoreThresholdRetriever(store, **kwargs)
    assert ScoreThresholdRetriever(store, score_threshold=1.0).invoke("q") == []
