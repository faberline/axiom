import pytest

from candidate import JsonOutputParser, OutputParserException, parse_with_retry

SCHEMA = {"name": str, "age": int, "active": bool}


def scripted(*replies):
    prompts = []

    def llm(prompt):
        prompts.append(prompt)
        return replies[len(prompts) - 1]

    return llm, prompts


def test_plain_and_fenced_json_parse():
    parser = JsonOutputParser(SCHEMA)
    obj = {"name": "a", "age": 3, "active": True}
    assert parser.parse('{"name": "a", "age": 3, "active": true}') == obj
    fenced = 'Sure!\n```json\n{"name": "a", "age": 3, "active": true}\n```\nDone.'
    assert parser.parse(fenced) == obj


def test_types_are_checked_strictly():
    parser = JsonOutputParser(SCHEMA)
    with pytest.raises(OutputParserException, match="'age' must be int"):
        parser.parse('{"name": "a", "age": true, "active": true}')
    with pytest.raises(OutputParserException, match="'active' must be bool"):
        parser.parse('{"name": "a", "age": 1, "active": 1}')
    with pytest.raises(OutputParserException, match="'name'"):
        parser.parse('{"age": 1, "active": false}')


def test_non_objects_and_bad_json_raise_with_output():
    parser = JsonOutputParser(SCHEMA)
    with pytest.raises(OutputParserException, match="object") as info:
        parser.parse("[1, 2]")
    assert info.value.llm_output == "[1, 2]"
    with pytest.raises(OutputParserException, match="invalid JSON"):
        parser.parse("{name: a}")


def test_retry_feeds_back_the_error_and_output():
    llm, prompts = scripted("oops", '{"name": "b", "age": 9, "active": false}')
    out = parse_with_retry(llm, JsonOutputParser(SCHEMA), "Extract the user.")
    assert out == {"name": "b", "age": 9, "active": False}
    assert len(prompts) == 2
    assert '"age": int' in prompts[0]
    assert "oops" in prompts[1] and "invalid JSON" in prompts[1]


def test_retries_are_bounded():
    llm, prompts = scripted("x", "y", "z", '{"name": "b", "age": 9, "active": false}')
    with pytest.raises(OutputParserException) as info:
        parse_with_retry(llm, JsonOutputParser(SCHEMA), "p", max_retries=2)
    assert len(prompts) == 3
    assert info.value.llm_output == "z"


def test_zero_retries_calls_once():
    llm, prompts = scripted("bad")
    with pytest.raises(OutputParserException):
        parse_with_retry(llm, JsonOutputParser(SCHEMA), "p", max_retries=0)
    assert len(prompts) == 1
