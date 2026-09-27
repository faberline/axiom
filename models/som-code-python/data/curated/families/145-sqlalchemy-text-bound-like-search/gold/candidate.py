"""User search through a raw SQL text() query with bound parameters."""

from sqlalchemy import Connection, text

SEARCH = text(
    "SELECT id, email FROM users WHERE email LIKE :pattern ESCAPE '\\' "
    "ORDER BY email LIMIT :limit"
)


def escape_like(term: str) -> str:
    """Make LIKE wildcards in term match literally."""
    return term.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_")


def search_users(
    conn: Connection, term: str, *, limit: int = 10
) -> list[tuple[int, str]]:
    """Return (id, email) for users whose email contains term."""
    if not term.strip():
        raise ValueError("search term must not be blank")
    if not 1 <= limit <= 50:
        raise ValueError("limit must be between 1 and 50")
    pattern = f"%{escape_like(term)}%"
    rows = conn.execute(SEARCH, {"pattern": pattern, "limit": limit})
    return [(row.id, row.email) for row in rows]
