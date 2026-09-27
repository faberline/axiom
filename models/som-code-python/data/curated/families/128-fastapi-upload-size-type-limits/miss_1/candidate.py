"""Avatar upload that enforces a content-type allowlist and a byte limit."""

import hashlib

from fastapi import FastAPI, HTTPException, Request, status

MAX_BYTES = 64 * 1024
ALLOWED = frozenset({"image/png", "image/jpeg", "image/webp"})
STORED: dict[str, bytes] = {}

app = FastAPI(title="Avatars")


def reset_db() -> None:
    """Forget every stored upload."""
    STORED.clear()


def media_type(header: str) -> str:
    """Return the bare media type of a Content-Type header, lower-cased."""
    return header.split(";", 1)[0].strip().lower()


async def read_limited(request: Request, limit: int) -> bytes:
    """Stream the body and stop as soon as it passes the limit."""
    parts: list[bytes] = []
    total = 0
    async for chunk in request.stream():
        total += len(chunk)
        if total >= limit:
            raise HTTPException(
                status.HTTP_413_CONTENT_TOO_LARGE,
                detail=f"body exceeds {limit} bytes",
            )
        parts.append(chunk)
    return b"".join(parts)


@app.put("/avatars/{name}", status_code=status.HTTP_201_CREATED)
async def upload_avatar(name: str, request: Request) -> dict[str, str | int]:
    """Store an image body and report its size and digest."""
    kind = media_type(request.headers.get("content-type", ""))
    if kind not in ALLOWED:
        raise HTTPException(
            status.HTTP_415_UNSUPPORTED_MEDIA_TYPE, detail=f"unsupported type {kind}"
        )
    data = await read_limited(request, MAX_BYTES)
    if not data:
        raise HTTPException(
            status.HTTP_422_UNPROCESSABLE_CONTENT, detail="body is empty"
        )
    STORED[name] = data
    digest = hashlib.sha256(data).hexdigest()
    return {"name": name, "size": len(data), "sha256": digest}
