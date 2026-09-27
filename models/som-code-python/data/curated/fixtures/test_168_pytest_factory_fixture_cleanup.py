import pytest

from candidate import AccountStore, make_account

__all__ = ["make_account"]


@pytest.fixture(scope="module")
def store():
    return AccountStore()


def test_factory_generates_names(make_account, store):
    first = make_account()
    second = make_account()
    assert store.accounts == {first: "user-1", second: "user-2"}


def test_teardown_deletes_newest_first_and_names_restart(make_account, store):
    assert store.accounts == {}
    assert store.deleted == [2, 1]
    account_id = make_account()
    assert store.accounts == {account_id: "user-1"}


def test_explicit_names_and_rejections(make_account, store):
    assert store.deleted == [2, 1, 3]
    ann = make_account("ann")
    assert store.accounts == {ann: "ann"}
    with pytest.raises(ValueError, match="duplicate account: ann"):
        make_account("ann")
    with pytest.raises(ValueError, match="blank"):
        make_account("   ")
    with pytest.raises(ValueError, match="blank"):
        make_account("")


def test_failed_creations_leave_nothing_and_manual_deletes_are_skipped(
    make_account, store
):
    assert store.accounts == {}
    assert store.deleted == [2, 1, 3, 4]
    gone = make_account("gone")
    store.delete(gone)
    make_account("kept")


def test_everything_was_cleaned_up(store):
    assert store.accounts == {}
    assert store.deleted == [2, 1, 3, 4, 5, 6]
