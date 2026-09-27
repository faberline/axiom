"""Documents updated under optimistic locking with If-Match and version ETags."""

from typing import Annotated

from fastapi import FastAPI, Header, HTTPException, Response, status
from pydantic import BaseModel, Field


class DocIn(BaseModel):
    """Fields a client may replace."""

    title: str = Field(min_length=1, max_length=200)


class DocOut(BaseModel):
    """A stored document and its version."""

    title: str
    version: int


DOCS: dict[int, DocOut] = {}

app = FastAPI(title="Documents")


def reset_db() -> None:
    """Restore the single seed document."""
    DOCS.clear()
    DOCS[1] = DocOut(title="Draft", version=1)


def etag(doc: DocOut) -> str:
    """Return the strong entity tag for a version."""
    return f'"v{doc.version}"'


def load(doc_id: int) -> DocOut:
    """Return a document or raise 404."""
    doc = DOCS.get(doc_id)
    if doc is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, detail="document not found")
    return doc


@app.get("/docs/{doc_id}")
def read_doc(doc_id: int, response: Response) -> DocOut:
    """Return a document with its ETag."""
    doc = load(doc_id)
    response.headers["ETag"] = etag(doc)
    return doc


@app.put("/docs/{doc_id}")
def update_doc(
    doc_id: int,
    payload: DocIn,
    response: Response,
    if_match: Annotated[str | None, Header()] = None,
) -> DocOut:
    """Replace a document only if the caller saw its current version."""
    doc = load(doc_id)
    if if_match is None:
        raise HTTPException(
            status.HTTP_428_PRECONDITION_REQUIRED, detail="If-Match header required"
        )
    if if_match.strip() != etag(doc):
        raise HTTPException(
            status.HTTP_412_PRECONDITION_FAILED,
            detail=f"document changed; current version is {doc.version}",
        )
    updated = DocOut(title=payload.title, version=doc.version + 1)
    DOCS[doc_id] = updated
    response.headers["ETag"] = etag(updated)
    return updated


reset_db()
