import json

import pytest

from candidate import DuplicateCustomIdError, build_batch, read_results

HELLO = [{"role": "user", "content": "hello"}]


def test_each_request_becomes_one_jsonl_line():
    text = build_batch([("a", HELLO), ("b", HELLO)], model="gpt-4o-mini")
    assert text.endswith("\n")
    lines = text.splitlines()
    assert len(lines) == 2
    first = json.loads(lines[0])
    assert first == {
        "custom_id": "a",
        "method": "POST",
        "url": "/v1/chat/completions",
        "body": {"model": "gpt-4o-mini", "messages": HELLO},
    }
    assert json.loads(lines[1])["custom_id"] == "b"


def test_empty_batch_is_empty_text():
    assert build_batch([], model="m") == ""


def test_duplicate_custom_ids_are_rejected():
    with pytest.raises(DuplicateCustomIdError, match="'a'"):
        build_batch([("a", HELLO), ("b", HELLO), ("a", HELLO)], model="m")


def ok(custom_id, content):
    body = {"choices": [{"message": {"role": "assistant", "content": content}}]}
    return {"custom_id": custom_id, "response": {"status_code": 200, "body": body}}


def test_results_split_answers_and_errors():
    records = [
        ok("a", "hi"),
        {
            "custom_id": "b",
            "response": {
                "status_code": 400,
                "body": {"error": {"message": "bad model"}},
            },
        },
        {"custom_id": "c", "response": None, "error": {"message": "expired"}},
        ok("d", "yo"),
    ]
    text = json.dumps(records[0]) + "\n\n" + "\n".join(map(json.dumps, records[1:]))
    answers, errors = read_results(text + "\n")
    assert answers == {"a": "hi", "d": "yo"}
    assert errors == {"b": "bad model", "c": "expired"}
