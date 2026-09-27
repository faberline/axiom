import pytest

from candidate import Message, Prompt, PromptArgument, PromptError


def review_prompt():
    return Prompt(
        "review",
        "Review this {language} code in a {tone} tone:\n{code}",
        [
            PromptArgument("code"),
            PromptArgument("language", required=False, default="Python"),
            PromptArgument("tone", required=False),
        ],
    )


def test_render_fills_values_and_defaults():
    messages = review_prompt().render({"code": "x = 1", "tone": "kind"})
    assert messages == [
        Message("user", "Review this Python code in a kind tone:\nx = 1")
    ]


def test_optional_argument_without_default_is_empty():
    content = review_prompt().render({"code": "y"})[0].content
    assert content == "Review this Python code in a  tone:\ny"


def test_values_are_not_reinterpreted_and_braces_escape():
    prompt = Prompt("json", "Return {{json}} for {q}", [PromptArgument("q")])
    assert prompt.render({"q": "{secret}"})[0].content == "Return {json} for {secret}"


def test_render_rejects_unknown_and_missing_arguments():
    prompt = review_prompt()
    with pytest.raises(PromptError, match="unknown arguments: extra"):
        prompt.render({"code": "x", "extra": "1"})
    with pytest.raises(PromptError, match="missing required arguments: code"):
        prompt.render({"tone": "kind"})


def test_declaration_is_checked_against_the_template():
    with pytest.raises(PromptError, match="undeclared arguments: topic"):
        Prompt("p", "Explain {topic}", [])
    with pytest.raises(PromptError):
        Prompt("p", "Explain {}", [])
    with pytest.raises(PromptError, match="role"):
        Prompt("p", "hi", [], role="system")
    assert Prompt("p", "hi", [], role="assistant").render({})[0].role == "assistant"


def test_describe_lists_arguments():
    assert review_prompt().describe() == {
        "name": "review",
        "arguments": [
            {"name": "code", "required": True},
            {"name": "language", "required": False},
            {"name": "tone", "required": False},
        ],
    }
