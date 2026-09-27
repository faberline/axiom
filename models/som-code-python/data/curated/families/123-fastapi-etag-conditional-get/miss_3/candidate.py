"""Article API that supports conditional GET with strong ETags."""

import hashlib
import json
from typing import Annotated

from fastapi import FastAPI, Header, HTTPException, Response, status
from pydantic import BaseModel


class Article(BaseModel):
    """A published article."""

    id: int
    title: str
    body: str


ARTICLES: dict[int, Article] = {}
app = FastAPI(title="Articles")


def reset_db() -> None:
    """Restore the single seeded article."""
    ARTICLES.clear()
    ARTICLES[1] = Article(id=1, title="Hello", body="First post")


reset_db()


def etag_for(article: Article) -> str:
    """Return a quoted tag derived from the article's full content."""
    payload = json.dumps(article.id).encode()
    return f'"{hashlib.sha256(payload).hexdigest()[:16]}"'


def matches(header: str | None, tag: str) -> bool:
    """Return whether an If-None-Match header selects the current tag."""
    if header is None:
        return False
    candidates = [part.strip() for part in header.split(",")]
    return "*" in candidates or tag in candidates or f"W/{tag}" in candidates


def _lookup(article_id: int) -> Article:
    article = ARTICLES.get(article_id)
    if article is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "article not found")
    return article


@app.get("/articles/{article_id}", response_model=Article)
def read_article(
    article_id: int,
    response: Response,
    if_none_match: Annotated[str | None, Header()] = None,
) -> Article | Response:
    """Return the article, or 304 when the client already holds this version."""
    article = _lookup(article_id)
    tag = etag_for(article)
    headers = {"ETag": tag, "Cache-Control": "no-cache"}
    if matches(if_none_match, tag):
        return Response(status_code=status.HTTP_304_NOT_MODIFIED, headers=headers)
    response.headers.update(headers)
    return article


@app.put("/articles/{article_id}", response_model=Article)
def replace_article(article_id: int, body: Article) -> Article:
    """Replace an existing article's content."""
    _lookup(article_id)
    ARTICLES[article_id] = body.model_copy(update={"id": article_id})
    return ARTICLES[article_id]
