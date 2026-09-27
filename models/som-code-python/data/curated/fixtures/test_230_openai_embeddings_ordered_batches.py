import pytest

from candidate import EmbeddingCountError, embed_all


class Embeddings:
    def __init__(self, drop=0):
        self.calls = []
        self.drop = drop

    def create(self, *, model, input):
        self.calls.append((model, list(input)))
        data = [
            {"index": i, "embedding": [float(ord(text[0]))]}
            for i, text in enumerate(input)
        ]
        data.reverse()
        return {"data": data[self.drop :]}


class Client:
    def __init__(self, drop=0):
        self.embeddings = Embeddings(drop)


TEXTS = ["a", "b", "c", "d", "e", "f", "g"]


def test_texts_are_sent_in_bounded_batches():
    client = Client()
    embed_all(client, TEXTS, model="text-embedding-3-small", batch_size=3)
    assert [len(batch) for _, batch in client.embeddings.calls] == [3, 3, 1]
    assert {model for model, _ in client.embeddings.calls} == {"text-embedding-3-small"}


def test_vectors_follow_input_order_despite_shuffled_response():
    vectors = embed_all(Client(), TEXTS, model="m", batch_size=3)
    assert vectors == [[float(ord(t))] for t in TEXTS]


def test_empty_input_makes_no_calls():
    client = Client()
    assert embed_all(client, [], model="m") == []
    assert client.embeddings.calls == []


def test_blank_text_is_rejected_before_any_call():
    client = Client()
    with pytest.raises(ValueError, match="non-empty"):
        embed_all(client, ["ok", "   "], model="m")
    assert client.embeddings.calls == []


def test_non_positive_batch_size_is_rejected():
    with pytest.raises(ValueError, match="batch_size must be positive"):
        embed_all(Client(), TEXTS, model="m", batch_size=0)


def test_short_response_raises():
    with pytest.raises(EmbeddingCountError, match="expected 2 embeddings, got 1"):
        embed_all(Client(drop=1), ["x", "y"], model="m")
