"""Telemetry ingestion that writes a whole batch in one INSERT ... RETURNING."""

from collections.abc import Generator
from typing import Any, ClassVar

from fastapi import Depends, FastAPI, HTTPException
from pydantic import BaseModel
from sqlalchemy import (
    create_engine,
    event,
    func,
    insert,
    select,
)
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, sessionmaker
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)


class QueryCounter:
    """Count the INSERT statements the engine executes."""

    insert_count: ClassVar[int] = 0
    statements: ClassVar[list[str]] = []

    @classmethod
    def reset(cls) -> None:
        """Start counting from zero."""
        cls.insert_count = 0
        cls.statements = []


@event.listens_for(engine, "before_cursor_execute", named=True)
def before_cursor_execute(**kw: Any) -> None:
    """Record every INSERT the engine sends to the database."""
    statement: str = kw["statement"]
    if statement.strip().upper().startswith("INSERT"):
        QueryCounter.insert_count += 1
        QueryCounter.statements.append(statement)


class Base(DeclarativeBase):
    """Declarative base shared by every table in this module."""


class TelemetryReading(Base):
    """One metric sample reported by a device."""

    __tablename__ = "telemetry_readings"

    id: Mapped[int] = mapped_column(primary_key=True, autoincrement=True)
    device_id: Mapped[str] = mapped_column(nullable=False)
    metric_name: Mapped[str] = mapped_column(nullable=False)
    value: Mapped[float] = mapped_column(nullable=False)
    timestamp: Mapped[str] = mapped_column(nullable=False)


Base.metadata.create_all(bind=engine)
session_factory = sessionmaker(autocommit=False, autoflush=False, bind=engine)


def get_db() -> Generator[Session, None, None]:
    """Yield one database session per request and close it afterwards."""
    db = session_factory()
    try:
        yield db
    finally:
        db.close()


def reset_db() -> None:
    """Recreate the tables and zero the counter."""
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    QueryCounter.reset()


reset_db()


class TelemetryReadingCreate(BaseModel):
    """One reading in a bulk upload."""

    device_id: str
    metric_name: str
    value: float
    timestamp: str


class BulkTelemetryRequest(BaseModel):
    """Payload accepted when uploading readings in bulk."""

    readings: list[TelemetryReadingCreate]


class BulkTelemetryResponse(BaseModel):
    """How many readings were stored and their ids."""

    inserted_count: int
    ids: list[int]


app = FastAPI()


@app.get("/query-metrics")
def get_query_metrics() -> dict[str, int]:
    """Return how many INSERT statements have run since the last reset."""
    return {"insert_count": QueryCounter.insert_count}


@app.get("/telemetry")
def get_telemetry(db: Session = Depends(get_db)) -> dict[str, int | None]:
    """Return how many readings are stored."""
    total = db.scalar(select(func.count(TelemetryReading.id)))
    return {"total": total}


@app.post("/telemetry/bulk", status_code=201, response_model=BulkTelemetryResponse)
def bulk_insert_telemetry(
    payload: BulkTelemetryRequest, db: Session = Depends(get_db)
) -> BulkTelemetryResponse:
    """Insert every reading in one statement and return the new ids."""
    if not payload.readings:
        raise HTTPException(status_code=400, detail="Readings payload cannot be empty")
    values = [r.model_dump() for r in payload.readings]
    inserted_ids = []
    for item in values:
        stmt = insert(TelemetryReading).values(**item).returning(TelemetryReading.id)
        res = db.execute(stmt)
        inserted_ids.append(res.scalar_one())
    db.commit()
    return BulkTelemetryResponse(inserted_count=len(inserted_ids), ids=inserted_ids)
