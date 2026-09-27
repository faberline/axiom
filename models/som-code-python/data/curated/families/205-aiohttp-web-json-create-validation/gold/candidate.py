"""A JSON create endpoint for aiohttp.web with explicit validation."""

from __future__ import annotations

import json
from typing import Any

from aiohttp import web

MAX_TITLE_CHARS = 100
NOTES_KEY = web.AppKey("notes", list[dict[str, Any]])


def _bad_request(message: str) -> web.Response:
    return web.json_response({"error": message}, status=400)


async def create_note(request: web.Request) -> web.Response:
    """Validate a JSON note, store it, and answer 201 with its location."""
    try:
        payload = await request.json()
    except json.JSONDecodeError:
        return _bad_request("body must be valid JSON")
    if not isinstance(payload, dict):
        return _bad_request("body must be a JSON object")
    title = payload.get("title")
    if not isinstance(title, str) or not title.strip():
        return _bad_request("title is required")
    if len(title) > MAX_TITLE_CHARS:
        return _bad_request(f"title must be at most {MAX_TITLE_CHARS} characters")
    tags = payload.get("tags", [])
    if not isinstance(tags, list) or not all(isinstance(tag, str) for tag in tags):
        return _bad_request("tags must be a list of strings")
    notes = request.app[NOTES_KEY]
    note = {"id": len(notes) + 1, "title": title.strip(), "tags": tags}
    notes.append(note)
    location = f"/notes/{note['id']}"
    return web.json_response(note, status=201, headers={"Location": location})


def make_app() -> web.Application:
    """Build the application with an empty note store."""
    app = web.Application()
    app[NOTES_KEY] = []
    app.router.add_post("/notes", create_note)
    return app
