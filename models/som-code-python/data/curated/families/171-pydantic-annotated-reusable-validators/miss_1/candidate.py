"""Reusable Annotated types that normalize a search query's fields."""

from typing import Annotated

from pydantic import (
    AfterValidator,
    BaseModel,
    BeforeValidator,
    Field,
    PlainSerializer,
)

MAX_TAGS = 5


def _split_csv(value: object) -> object:
    """Accept "a, b" strings as well as lists; drop blank parts."""
    if isinstance(value, str):
        return [part.strip() for part in value.split(",")]
    return value


def _normalize_tags(tags: list[str]) -> list[str]:
    """Lower-case, de-duplicate and sort tags, capping how many there are."""
    unique = sorted({tag.lower() for tag in tags})
    if len(unique) > MAX_TAGS:
        raise ValueError(f"at most {MAX_TAGS} distinct tags")
    return unique


def _lower(value: str) -> str:
    return value.lower()


Tags = Annotated[
    list[str],
    BeforeValidator(_split_csv),
    AfterValidator(_normalize_tags),
    PlainSerializer(",".join, return_type=str),
]
Username = Annotated[str, Field(min_length=3, max_length=20), AfterValidator(_lower)]


class SearchQuery(BaseModel):
    """Filters for a repository search."""

    owner: Username
    tags: Tags = Field(default_factory=list)
    exclude: Tags = Field(default_factory=list)
