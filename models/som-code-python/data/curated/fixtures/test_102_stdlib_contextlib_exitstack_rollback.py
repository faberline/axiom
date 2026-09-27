from contextlib import contextmanager

import pytest

from candidate import AcquireError, ResourceBundle


def tracked(name, log, fail=False):
    @contextmanager
    def manager():
        if fail:
            raise OSError(f"{name} unavailable")
        log.append(f"open {name}")
        try:
            yield name
        finally:
            log.append(f"close {name}")

    return manager


def test_acquires_all_and_closes_in_reverse():
    log = []
    with ResourceBundle.acquire([tracked("db", log), tracked("cache", log)]) as bundle:
        assert bundle.resources == ["db", "cache"]
        assert log == ["open db", "open cache"]
    assert log == ["open db", "open cache", "close cache", "close db"]


def test_failure_releases_already_opened():
    log = []
    factories = [tracked("db", log), tracked("cache", log), tracked("queue", log, fail=True)]
    with pytest.raises(AcquireError, match="resource 2 failed") as info:
        ResourceBundle.acquire(factories)
    assert log == ["open db", "open cache", "close cache", "close db"]
    assert isinstance(info.value.__cause__, OSError)


def test_resources_stay_open_until_close():
    log = []
    bundle = ResourceBundle.acquire([tracked("db", log)])
    assert log == ["open db"]
    bundle.close()
    assert log == ["open db", "close db"]


def test_rejects_empty_factory_list():
    with pytest.raises(ValueError, match="at least one"):
        ResourceBundle.acquire([])
