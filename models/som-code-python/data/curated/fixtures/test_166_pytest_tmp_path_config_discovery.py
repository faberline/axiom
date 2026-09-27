import pytest

from candidate import find_config, make_tree

__all__ = ["make_tree"]


def test_nearest_config_wins(make_tree):
    root = make_tree({"app.toml": "root", "pkg/app.toml": "pkg", "pkg/sub/mod.py": ""})
    assert find_config(root / "pkg" / "sub") == (root / "pkg" / "app.toml").resolve()
    assert find_config(root / "pkg") == (root / "pkg" / "app.toml").resolve()


def test_start_may_be_a_file(make_tree):
    root = make_tree({"app.toml": "root", "pkg/sub/mod.py": ""})
    assert find_config(root / "pkg" / "sub" / "mod.py") == (root / "app.toml").resolve()


def test_directories_with_the_name_are_ignored(make_tree):
    root = make_tree({"app.toml": "root", "a/app.toml/": "", "a/b/": ""})
    assert find_config(root / "a" / "b") == (root / "app.toml").resolve()


def test_search_stops_at_the_repository_root(make_tree):
    root = make_tree({"app.toml": "outer", "repo/.git/": "", "repo/src/": ""})
    assert find_config(root / "repo" / "src") is None


def test_config_beside_git_is_still_found(make_tree):
    root = make_tree({"repo/.git/": "", "repo/app.toml": "inner", "repo/src/": ""})
    assert find_config(root / "repo" / "src") == (root / "repo" / "app.toml").resolve()


def test_custom_name(make_tree):
    root = make_tree({"pyproject.toml": "", "x/": ""})
    assert (
        find_config(root / "x", "pyproject.toml") == (root / "pyproject.toml").resolve()
    )


@pytest.mark.parametrize("name", ["", "conf/app.toml"])
def test_invalid_names(tmp_path, name):
    with pytest.raises(ValueError, match="plain file name"):
        find_config(tmp_path, name)
