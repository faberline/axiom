"""Oracle test suite for 40-redis-pubsub-pipeline with InMemoryRedis isolation."""
import pytest
from redis.exceptions import WatchError
import candidate


class InMemoryPipeline:
    def __init__(self, client, transaction=True):
        self.client = client
        self.transaction = transaction
        self.watched_versions = {}
        self.commands = []
        self.in_multi = False
        self.reset_called = False

    def watch(self, *keys):
        for k in keys:
            self.watched_versions[k] = self.client.versions.get(k, 0)

    def multi(self):
        self.in_multi = True

    def set(self, key, val):
        self.commands.append(("set", key, val))

    def get(self, key):
        self.commands.append(("get", key))

    def execute(self):
        for k, v in self.watched_versions.items():
            if self.client.versions.get(k, 0) != v:
                raise WatchError("Watched variable changed.")
        results = []
        for cmd in self.commands:
            if cmd[0] == "set":
                self.client.store[cmd[1]] = cmd[2]
                self.client.versions[cmd[1]] = self.client.versions.get(cmd[1], 0) + 1
                results.append(True)
            elif cmd[0] == "get":
                results.append(self.client.store.get(cmd[1]))
        self.commands.clear()
        return results

    def reset(self):
        self.reset_called = True
        self.watched_versions.clear()
        self.commands.clear()
        self.in_multi = False


class InMemoryPubSub:
    def __init__(self, client):
        self.client = client
        self.subscribed_channels = set()
        self.closed = False

    def subscribe(self, *channels):
        for ch in channels:
            self.subscribed_channels.add(ch)
            self.client.channel_subscribers.setdefault(ch, set()).add(self)

    def unsubscribe(self, *channels):
        for ch in channels:
            self.subscribed_channels.discard(ch)
            if ch in self.client.channel_subscribers:
                self.client.channel_subscribers[ch].discard(self)

    def close(self):
        self.closed = True
        for ch in list(self.subscribed_channels):
            self.unsubscribe(ch)


class InMemoryRedis:
    def __init__(self):
        self.store = {}
        self.versions = {}
        self.channel_subscribers = {}
        self.last_pipeline = None

    def pipeline(self, transaction=True):
        pipe = InMemoryPipeline(self, transaction=transaction)
        self.last_pipeline = pipe
        return pipe

    def pubsub(self):
        return InMemoryPubSub(self)

    def publish(self, channel, message):
        subs = self.channel_subscribers.get(channel, set())
        return len(subs)


def test_transaction_pipeline_default_retries():
    client = InMemoryRedis()
    tx = candidate.RedisTransactionPipeline(client)
    assert tx.max_retries == 3


def test_empty_watch_keys_rejected():
    client = InMemoryRedis()
    tx = candidate.RedisTransactionPipeline(client)
    with pytest.raises(ValueError, match="watch_keys"):
        tx.execute_transaction([], lambda pipe: None)


def test_watch_error_retry_attempts():
    client = InMemoryRedis()
    tx = candidate.RedisTransactionPipeline(client, max_retries=1)
    attempts = [0]

    def callback(pipe):
        attempts[0] += 1
        if attempts[0] == 1:
            client.versions["balance"] = client.versions.get("balance", 0) + 1
        pipe.set("balance", 100)

    res = tx.execute_transaction(["balance"], callback)
    assert res == [True]
    assert attempts[0] == 2


def test_pubsub_dispatch_message_type():
    client = InMemoryRedis()
    mgr = candidate.RedisPubSubManager(client)
    received = []
    mgr.subscribe("metrics", lambda ch, data: received.append((ch, data)))

    msg = {"type": "message", "channel": "metrics", "data": "cpu_load_80"}
    dispatched = mgr.dispatch_message(msg)
    assert dispatched == 1
    assert received == [("metrics", "cpu_load_80")]
    mgr.close()


def test_pipeline_finally_resets():
    client = InMemoryRedis()
    tx = candidate.RedisTransactionPipeline(client)
    tx.execute_transaction(["key1"], lambda pipe: pipe.set("key1", "val1"))
    assert client.last_pipeline is not None
    assert client.last_pipeline.reset_called is True
