import pytest

from candidate import ResourceError, ResourceRegistry, ResourceTemplate


def make_registry():
    registry = ResourceRegistry()
    registry.add_resource("config://app", lambda: "debug=false")
    registry.add_resource("users://me/profile", lambda: "static me")
    registry.add_template(
        "users://{user_id}/profile", lambda user_id: f"user {user_id}"
    )
    registry.add_template(
        "repo://{owner}/{name}/readme", lambda owner, name: f"{owner}/{name}"
    )
    registry.add_template("files://{stem}.txt", lambda stem: f"file {stem}")
    return registry


def test_static_and_template_resources_resolve():
    registry = make_registry()
    assert registry.read_resource("config://app") == "debug=false"
    assert registry.read_resource("users://42/profile") == "user 42"
    assert registry.read_resource("repo://octo/cat/readme") == "octo/cat"
    assert registry.read_resource("files://report.txt") == "file report"


def test_exact_resources_win_over_templates():
    assert make_registry().read_resource("users://me/profile") == "static me"


def test_parameters_are_percent_decoded_but_plus_is_literal():
    registry = make_registry()
    assert registry.read_resource("users://j%20doe/profile") == "user j doe"
    assert registry.read_resource("users://a+b/profile") == "user a+b"


def test_whole_uri_must_match_without_crossing_slashes():
    registry = make_registry()
    for uri in (
        "users://a/b/profile",
        "users://42/profile/extra",
        "files://reportXtxt",
        "config://app/x",
    ):
        with pytest.raises(ResourceError, match="not found"):
            registry.read_resource(uri)


def test_template_params_and_duplicates():
    template = ResourceTemplate("repo://{owner}/{name}", lambda owner, name: "")
    assert template.params == ("owner", "name")
    assert template.match("repo://a/b") == {"owner": "a", "name": "b"}
    with pytest.raises(ResourceError, match="duplicate"):
        ResourceTemplate("x://{id}/{id}", lambda id: "")
