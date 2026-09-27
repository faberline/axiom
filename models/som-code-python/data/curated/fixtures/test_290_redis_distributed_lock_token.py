import pytest

from candidate import Lock, LockError


class FakeRedis:
    def __init__(self):
        self.now = 0
        self.data = {}

    def _live(self, name):
        item = self.data.get(name)
        if item and item[1] is not None and item[1] <= self.now:
            del self.data[name]
            return None
        return item

    def set(self, name, value, *, nx=False, px=None):
        if nx and self._live(name):
            return None
        self.data[name] = (value, None if px is None else self.now + px)
        return True

    def delete(self, name):
        return 1 if self.data.pop(name, None) else 0

    def eval(self, script, numkeys, *args):
        name, token = args[0], args[1]
        item = self._live(name)
        if not item or item[0] != token:
            return 0
        if "pexpire" in script:
            self.data[name] = (item[0], self.now + int(args[2]))
            return 1
        del self.data[name]
        return 1


@pytest.fixture
def redis():
    return FakeRedis()


def test_mutual_exclusion_and_release(redis):
    a, b = Lock(redis, "job", 1000), Lock(redis, "job", 1000)
    assert a.acquire()
    assert not b.acquire()
    a.release()
    assert b.acquire()
    assert redis.data["lock:job"][1] == 1000


def test_expired_lock_cannot_delete_new_holder(redis):
    a, b = Lock(redis, "job", 100), Lock(redis, "job", 100)
    assert a.acquire()
    redis.now = 150
    assert b.acquire()
    with pytest.raises(LockError, match="expired"):
        a.release()
    assert "lock:job" in redis.data
    b.release()


def test_release_twice_reports_not_held(redis):
    lock = Lock(redis, "job", 100)
    assert lock.acquire()
    lock.release()
    with pytest.raises(LockError, match="not held"):
        lock.release()


def test_extend_keeps_the_lock_alive(redis):
    lock = Lock(redis, "job", 100)
    assert lock.acquire()
    redis.now = 90
    lock.extend()
    redis.now = 150
    assert not Lock(redis, "job", 100).acquire()
    redis.now = 300
    with pytest.raises(LockError):
        lock.extend()


def test_context_manager(redis):
    holder = Lock(redis, "job", 100)
    with holder:
        with pytest.raises(LockError, match="busy"):
            with Lock(redis, "job", 100):
                pass
    assert "lock:job" not in redis.data
    with pytest.raises(ValueError):
        Lock(redis, "job", 0)
