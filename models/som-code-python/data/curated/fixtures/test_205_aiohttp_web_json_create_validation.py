import asyncio

import aiohttp
from aiohttp.test_utils import TestServer

from candidate import NOTES_KEY, make_app


def post_all(bodies):
    async def main():
        app = make_app()
        results = []
        async with TestServer(app) as server, aiohttp.ClientSession() as session:
            url = str(server.make_url("/notes"))
            for body in bodies:
                async with session.post(url, data=body) as resp:
                    results.append(
                        (resp.status, await resp.json(), resp.headers.get("Location"))
                    )
        return results, app[NOTES_KEY]

    return asyncio.run(main())


def test_valid_notes_are_created_with_ids_and_location():
    results, notes = post_all(
        ['{"title": "  Groceries ", "tags": ["home"]}', '{"title": "Taxes"}']
    )
    assert results[0] == (
        201,
        {"id": 1, "title": "Groceries", "tags": ["home"]},
        "/notes/1",
    )
    assert results[1] == (201, {"id": 2, "title": "Taxes", "tags": []}, "/notes/2")
    assert [note["title"] for note in notes] == ["Groceries", "Taxes"]


def test_title_length_boundary():
    results, notes = post_all(
        ['{"title": "%s"}' % ("a" * 100), '{"title": "%s"}' % ("b" * 101)]
    )
    assert results[0][0] == 201
    assert results[1][0] == 400
    assert results[1][1] == {"error": "title must be at most 100 characters"}
    assert len(notes) == 1


def test_malformed_bodies_are_400():
    results, notes = post_all(["{not json", "[1, 2]", ""])
    assert [status for status, _, _ in results] == [400, 400, 400]
    assert results[0][1] == {"error": "body must be valid JSON"}
    assert results[1][1] == {"error": "body must be a JSON object"}
    assert notes == []


def test_missing_or_blank_titles_are_400():
    results, _ = post_all(['{"tags": []}', '{"title": "   "}', '{"title": 7}'])
    assert all(
        result[:2] == (400, {"error": "title is required"}) for result in results
    )


def test_tags_must_be_strings():
    results, notes = post_all(
        ['{"title": "a", "tags": "home"}', '{"title": "b", "tags": ["x", 1]}']
    )
    expected = (400, {"error": "tags must be a list of strings"}, None)
    assert results == [expected, expected]
    assert notes == []
