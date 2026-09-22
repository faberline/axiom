from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from sqlalchemy import create_engine, Column, Integer, String, Float
from sqlalchemy.orm import DeclarativeBase, sessionmaker, Session
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)


class Base(DeclarativeBase):
    pass


class Account(Base):
    __tablename__ = "accounts"

    id = Column(Integer, primary_key=True, index=True)
    account_number = Column(String, unique=True, nullable=False)
    balance = Column(Float, nullable=False)


Base.metadata.create_all(bind=engine)
SyncSessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)


class AsyncSessionMetrics:
    active_sessions: int = 0
    total_created: int = 0
    total_closed: int = 0
    total_commits: int = 0
    total_rollbacks: int = 0
    total_flushes: int = 0

    @classmethod
    def reset(cls):
        cls.active_sessions = 0
        cls.total_created = 0
        cls.total_closed = 0
        cls.total_commits = 0
        cls.total_rollbacks = 0
        cls.total_flushes = 0


class AsyncSessionAdapter:
    def __init__(self, sync_session: Session):
        self._sync_session = sync_session
        self.is_active = True
        AsyncSessionMetrics.active_sessions += 1
        AsyncSessionMetrics.total_created += 1

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if exc_type is not None:
            await self.rollback()
        await self.close()

    async def get(self, entity, ident):
        return self._sync_session.get(entity, ident)

    async def commit(self):
        AsyncSessionMetrics.total_commits += 1
        self._sync_session.commit()

    async def rollback(self):
        AsyncSessionMetrics.total_rollbacks += 1
        self._sync_session.rollback()

    async def flush(self):
        AsyncSessionMetrics.total_flushes += 1
        self._sync_session.flush()

    async def close(self):
        if self.is_active:
            self.is_active = False
            AsyncSessionMetrics.active_sessions -= 1
            AsyncSessionMetrics.total_closed += 1
            self._sync_session.close()


def async_session_factory() -> AsyncSessionAdapter:
    return AsyncSessionAdapter(SyncSessionLocal())


def reset_db() -> None:
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    db = SyncSessionLocal()
    acc1 = Account(account_number="ACC-101", balance=1000.0)
    acc2 = Account(account_number="ACC-102", balance=500.0)
    db.add_all([acc1, acc2])
    db.commit()
    db.close()
    AsyncSessionMetrics.reset()


reset_db()


class TransferRequest(BaseModel):
    from_account_id: int
    to_account_id: int
    amount: float


class TransferResponse(BaseModel):
    status: str
    transferred: float
    from_balance: float
    to_balance: float


app = FastAPI()


@app.get("/session-metrics")
def get_session_metrics():
    return {
        "active_sessions": AsyncSessionMetrics.active_sessions,
        "total_created": AsyncSessionMetrics.total_created,
        "total_closed": AsyncSessionMetrics.total_closed,
        "total_commits": AsyncSessionMetrics.total_commits,
        "total_rollbacks": AsyncSessionMetrics.total_rollbacks,
        "total_flushes": AsyncSessionMetrics.total_flushes,
    }


@app.get("/accounts/{account_id}")
def get_account(account_id: int):
    db = SyncSessionLocal()
    try:
        acc = db.get(Account, account_id)
        if not acc:
            raise HTTPException(status_code=404, detail="Account not found")
        return {"id": acc.id, "account_number": acc.account_number, "balance": acc.balance}
    finally:
        db.close()


@app.post("/transfers", status_code=200, response_model=TransferResponse)
async def transfer_funds(payload: TransferRequest):
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
        await session.flush()
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
    except Exception:
        await session.rollback()
        await session.close()
        raise HTTPException(status_code=500, detail="Transfer failed")
