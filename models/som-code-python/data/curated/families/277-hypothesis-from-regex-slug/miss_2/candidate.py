"""URL slugs with a validator and a Hypothesis from_regex strategy."""

from __future__ import annotations

import re
import unicodedata

from hypothesis import strategies as st

SLUG_PATTERN = r"[a-z0-9]+(?:-[a-z0-9]+)*"
SLUG_RE = re.compile(SLUG_PATTERN)
MAX_LEN = 40


class SlugError(ValueError):
    """Raised when a title has no characters usable in a slug."""


def slugify(title: str) -> str:
    """Return a lowercase ASCII slug of at most MAX_LEN characters."""
    folded = unicodedata.normalize("NFKD", title)
    ascii_title = folded.encode("ascii", "ignore").decode("ascii")
    slug = re.sub(r"[^a-z0-9]+", "-", ascii_title.lower()).strip("-")
    slug = slug[:MAX_LEN]
    if not slug:
        raise SlugError(f"cannot build a slug from {title!r}")
    return slug


def is_slug(text: str) -> bool:
    """Return whether the whole text is a valid slug within MAX_LEN."""
    return len(text) <= MAX_LEN and SLUG_RE.fullmatch(text) is not None


def slugs() -> st.SearchStrategy[str]:
    """Return a strategy producing only valid slugs."""
    return st.from_regex(SLUG_PATTERN, fullmatch=True).filter(
        lambda text: len(text) <= MAX_LEN
    )
