"""Transfer funds through an async session adapter that counts its lifecycle."""

from types import TracebackType
from typing import Self

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from sqlalchemy import create_engine
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, sessionmaker
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)


class Base(DeclarativeBase):
    """Declarative base shared by every table in this module."""


class Account(Base):
    """A bank account with a balance."""

    __tablename__ = "accounts"

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    account_number: Mapped[str] = mapped_column(unique=True, nullable=False)
    balance: Mapped[float] = mapped_column(nullable=False)


Base.metadata.create_all(bind=engine)
sync_session_factory = sessionmaker(autocommit=False, autoflush=False, bind=engine)


class AsyncSessionMetrics:
    """Counters of every session opened, closed, committed, and rolled back."""

    active_sessions: int = 0
    total_created: int = 0
    total_closed: int = 0
    total_commits: int = 0
    total_rollbacks: int = 0
    total_flushes: int = 0

    @classmethod
    def reset(cls) -> None:
        """Set every counter back to zero."""
        cls.active_sessions = 0
        cls.total_created = 0
        cls.total_closed = 0
        cls.total_commits = 0
        cls.total_rollbacks = 0
        cls.total_flushes = 0


class AsyncSessionAdapter:
    """An awaitable facade over a synchronous session that feeds the metrics."""

    def __init__(self, sync_session: Session) -> None:
        self._sync_session = sync_session
        self.is_active = True
        AsyncSessionMetrics.active_sessions += 1
        AsyncSessionMetrics.total_created += 1

    async def __aenter__(self) -> Self:
        return self

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None:
        if exc_type is not None:
            await self.rollback()
        await self.close()

    async def get[T](self, entity: type[T], ident: int) -> T | None:
        """Load one row by primary key."""
        return self._sync_session.get(entity, ident)

    async def commit(self) -> None:
        """Commit the transaction."""
        AsyncSessionMetrics.total_commits += 1
        self._sync_session.commit()

    async def rollback(self) -> None:
        """Roll back the transaction."""
        AsyncSessionMetrics.total_rollbacks += 1
        self._sync_session.rollback()

    async def flush(self) -> None:
        """Flush pending changes without committing."""
        AsyncSessionMetrics.total_flushes += 1
        self._sync_session.flush()

    async def close(self) -> None:
        """Close the session once; later calls do nothing."""
        if self.is_active:
            self.is_active = False
            AsyncSessionMetrics.active_sessions -= 1
            AsyncSessionMetrics.total_closed += 1
            self._sync_session.close()


def async_session_factory() -> AsyncSessionAdapter:
    """Open a new adapter over a fresh synchronous session."""
    return AsyncSessionAdapter(sync_session_factory())


def reset_db() -> None:
    """Recreate the tables, seed two accounts, and zero the metrics."""
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    db = sync_session_factory()
    acc1 = Account(account_number="ACC-101", balance=1000.0)
    acc2 = Account(account_number="ACC-102", balance=500.0)
    db.add_all([acc1, acc2])
    db.commit()
    db.close()
    AsyncSessionMetrics.reset()


reset_db()


class TransferRequest(BaseModel):
    """Payload accepted when moving money between two accounts."""

    from_account_id: int
    to_account_id: int
    amount: float


class TransferResponse(BaseModel):
    """The outcome of a transfer with both resulting balances."""

    status: str
    transferred: float
    from_balance: float
    to_balance: float


app = FastAPI()


@app.get("/session-metrics")
def get_session_metrics() -> dict[str, int]:
    """Return the session lifecycle counters."""
    return {
        "active_sessions": AsyncSessionMetrics.active_sessions,
        "total_created": AsyncSessionMetrics.total_created,
        "total_closed": AsyncSessionMetrics.total_closed,
        "total_commits": AsyncSessionMetrics.total_commits,
        "total_rollbacks": AsyncSessionMetrics.total_rollbacks,
        "total_flushes": AsyncSessionMetrics.total_flushes,
    }


@app.get("/accounts/{account_id}")
def get_account(account_id: int) -> dict[str, int | str | float]:
    """Return one account, or 404 when it does not exist."""
    db = sync_session_factory()
    try:
        acc = db.get(Account, account_id)
        if not acc:
            raise HTTPException(status_code=404, detail="Account not found")
        return {
            "id": acc.id,
            "account_number": acc.account_number,
            "balance": acc.balance,
        }
    finally:
        db.close()


@app.post("/transfers", status_code=200, response_model=TransferResponse)
async def transfer_funds(payload: TransferRequest) -> TransferResponse:
    """Move money in one session, rolling back and closing it on any failure."""
    session = async_session_factory()
    try:
        sender = await session.get(Account, payload.from_account_id)
        receiver = await session.get(Account, payload.to_account_id)
        if not sender or not receiver:
            raise HTTPException(status_code=404, detail="Account not found")
        if payload.amount <= 0:
            raise HTTPException(status_code=400, detail="Invalid transfer amount")
        if sender.balance < payload.amount:
            raise HTTPException(status_code=400, detail="Insufficient funds")

        sender.balance -= payload.amount
        receiver.balance += payload.amount
        session.commit()
        res = TransferResponse(
            status="success",
            transferred=payload.amount,
            from_balance=sender.balance,
            to_balance=receiver.balance,
        )
        await session.close()
        return res
    except HTTPException:
        await session.rollback()
        await session.close()
        raise
    except Exception as exc:
        await session.rollback()
        await session.close()
        raise HTTPException(status_code=500, detail="Transfer failed") from exc
