import pytest

from candidate import TokenBucket


class FakeRedis:
    def __init__(self):
        self.hashes = {}
        self.ttls = {}

    def hmget(self, name, keys):
        stored = self.hashes.get(name, {})
        return [stored.get(k) for k in keys]

    def hset(self, name, mapping):
        self.hashes.setdefault(name, {}).update(
            {k: str(v).encode() for k, v in mapping.items()}
        )
        return len(mapping)

    def expire(self, name, seconds):
        self.ttls[name] = seconds
        return True


class Clock:
    def __init__(self):
        self.t = 1000.0

    def __call__(self):
        return self.t


@pytest.fixture
def setup():
    redis, clock = FakeRedis(), Clock()
    return redis, clock, TokenBucket(redis, capacity=3, rate=0.5, clock=clock)


def test_burst_then_refill(setup):
    _, clock, bucket = setup
    assert [bucket.allow("u") for _ in range(4)] == [True, True, True, False]
    assert bucket.retry_after("u") == pytest.approx(2.0)
    clock.t += 2
    assert bucket.allow("u")
    assert not bucket.allow("u")


def test_refill_is_capped_at_capacity(setup):
    _, clock, bucket = setup
    bucket.allow("u")
    clock.t += 3600
    assert [bucket.allow("u") for _ in range(4)] == [True, True, True, False]


def test_exact_balance_is_spendable_and_keys_are_separate(setup):
    _, _, bucket = setup
    assert bucket.allow("a", cost=3)
    assert not bucket.allow("a")
    assert bucket.allow("b", cost=3)


def test_clock_skew_does_not_drain_and_ttl_is_set(setup):
    redis, clock, bucket = setup
    bucket.allow("u")
    clock.t -= 100
    assert bucket.allow("u", cost=2)
    assert redis.ttls["bucket:u"] == 6


def test_invalid_arguments(setup):
    redis, _, bucket = setup
    for cost in (0, 4):
        with pytest.raises(ValueError):
            bucket.allow("u", cost=cost)
    with pytest.raises(ValueError):
        TokenBucket(redis, capacity=0, rate=1)
    with pytest.raises(ValueError):
        TokenBucket(redis, capacity=1, rate=0)
