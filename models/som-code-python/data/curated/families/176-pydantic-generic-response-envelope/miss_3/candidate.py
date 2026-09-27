"""A generic API envelope whose payload type is chosen at the call site."""

from pydantic import BaseModel, Field


class User(BaseModel):
    """A user record returned by the API."""

    id: int = Field(gt=0)
    name: str


class ApiError(BaseModel):
    """One error entry in a failed response."""

    code: str
    message: str


class Envelope[T](BaseModel):
    """Every API response: a request id, optional data and a list of errors."""

    request_id: str = Field(min_length=8)
    data: T | None = None
    errors: list[ApiError] = Field(default_factory=list)


class ApiResponseError(Exception):
    """The API answered with errors."""

    def __init__(self, request_id: str, errors: list[ApiError]) -> None:
        details = "; ".join(f"{error.code}: {error.message}" for error in errors)
        super().__init__(f"{request_id}: {details}")
        self.errors = errors


def _unwrap[T](envelope: Envelope[T]) -> T:
    if envelope.errors:
        raise ApiResponseError(envelope.request_id, envelope.errors)
    return envelope.data


def fetch_user(raw: str) -> User:
    """Parse a single-user response body."""
    return _unwrap(Envelope[User].model_validate_json(raw))


def fetch_users(raw: str) -> list[User]:
    """Parse a user-list response body."""
    return _unwrap(Envelope[list[User]].model_validate_json(raw))
