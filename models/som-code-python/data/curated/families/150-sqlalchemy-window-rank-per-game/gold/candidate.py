"""Build per-game leaderboards with a RANK() window function."""

from sqlalchemy import String, func, select
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column


class Base(DeclarativeBase):
    """Declarative base for the scoring tables."""


class Score(Base):
    """One player's points in one game."""

    __tablename__ = "scores"

    id: Mapped[int] = mapped_column(primary_key=True)
    player: Mapped[str] = mapped_column(String(50))
    game: Mapped[str] = mapped_column(String(50))
    points: Mapped[int]


def leaderboard(session: Session, *, top: int = 3) -> dict[str, list[tuple[str, int]]]:
    """Map each game to its players placed within top, ties sharing a place."""
    if top < 1:
        raise ValueError("top must be at least 1")
    pos = func.rank().over(partition_by=Score.game, order_by=Score.points.desc())
    ranked = select(Score.game, Score.player, pos.label("place")).subquery()
    stmt = (
        select(ranked.c.game, ranked.c.player, ranked.c.place)
        .where(ranked.c.place <= top)
        .order_by(ranked.c.game, ranked.c.place, ranked.c.player)
    )
    board: dict[str, list[tuple[str, int]]] = {}
    for game, player, position in session.execute(stmt):
        board.setdefault(game, []).append((player, position))
    return board
