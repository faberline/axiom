"""Apply user edits to a frozen profile model without skipping validation."""

from collections.abc import Mapping

from pydantic import BaseModel, ConfigDict, Field

EDITABLE = frozenset({"display_name", "tags"})


class Profile(BaseModel):
    """A public user profile; instances never change after validation."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    handle: str = Field(min_length=3, max_length=15)
    display_name: str = Field(min_length=1, max_length=50)
    tags: tuple[str, ...] = ()
    version: int = Field(default=1, ge=1)


def apply_update(profile: Profile, changes: Mapping[str, object]) -> Profile:
    """Return a re-validated copy with the edits applied and the version bumped."""
    unknown = set(changes) - EDITABLE
    if unknown:
        raise ValueError(f"fields not editable: {', '.join(sorted(unknown))}")
    if not changes:
        return profile
    return profile.model_copy(update={**changes, "version": profile.version + 1})
