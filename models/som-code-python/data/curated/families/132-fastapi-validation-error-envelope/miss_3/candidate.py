"""Return validation failures in one stable envelope without echoing input."""

from typing import Annotated

from fastapi import FastAPI, Query, Request, status
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field


class Signup(BaseModel):
    """A new account request."""

    email: str = Field(pattern=r"^[^@\s]+@[^@\s]+\.[a-z]{2,}$")
    age: int = Field(ge=13, le=130)


app = FastAPI(title="Signup")


def field_name(loc: tuple[int | str, ...]) -> str:
    """Drop the body prefix; keep query, path and header prefixes."""
    parts = [str(part) for part in loc]
    return ".".join(parts)


@app.exception_handler(RequestValidationError)
async def validation_envelope(
    request: Request, exc: RequestValidationError
) -> JSONResponse:
    """Answer 400 with field names and messages only."""
    del request
    errors = [
        {"field": field_name(tuple(err["loc"])), "message": str(err["msg"])}
        for err in exc.errors()
    ]
    return JSONResponse(
        status_code=status.HTTP_400_BAD_REQUEST,
        content={"code": "validation_failed", "errors": errors},
    )


@app.post("/signup", status_code=status.HTTP_201_CREATED)
def signup(payload: Signup) -> Signup:
    """Accept a valid signup."""
    return payload


@app.get("/users")
def list_users(limit: Annotated[int, Query(ge=1, le=100)] = 10) -> dict[str, int]:
    """Echo the accepted limit."""
    return {"limit": limit}
