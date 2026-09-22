from fastapi import FastAPI, HTTPException, Depends
from pydantic import BaseModel
from sqlalchemy import create_engine, Column, Integer, String, Float, insert, select, func, event
from sqlalchemy.orm import DeclarativeBase, sessionmaker, Session
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)


class QueryCounter:
    insert_count: int = 0
    statements: list[str] = []

    @classmethod
    def reset(cls):
        cls.insert_count = 0
        cls.statements = []


@event.listens_for(engine, "before_cursor_execute")
def before_cursor_execute(conn, cursor, statement, parameters, context, executemany):
    if statement.strip().upper().startswith("INSERT"):
        QueryCounter.insert_count += 1
        QueryCounter.statements.append(statement)


class Base(DeclarativeBase):
    pass


class TelemetryReading(Base):
    __tablename__ = "telemetry_readings"

    id = Column(Integer, primary_key=True, autoincrement=True)
    device_id = Column(String, nullable=False)
    metric_name = Column(String, nullable=False)
    value = Column(Float, nullable=False)
    timestamp = Column(String, nullable=False)


Base.metadata.create_all(bind=engine)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)


def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()


def reset_db() -> None:
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    QueryCounter.reset()


reset_db()


class TelemetryReadingCreate(BaseModel):
    device_id: str
    metric_name: str
    value: float
    timestamp: str


class BulkTelemetryRequest(BaseModel):
    readings: list[TelemetryReadingCreate]


class BulkTelemetryResponse(BaseModel):
    inserted_count: int
    ids: list[int]


app = FastAPI()


@app.get("/query-metrics")
def get_query_metrics():
    return {"insert_count": QueryCounter.insert_count}


@app.get("/telemetry")
def get_telemetry(db: Session = Depends(get_db)):
    total = db.scalar(select(func.count(TelemetryReading.id)))
    return {"total": total}


@app.post("/telemetry/bulk", status_code=201, response_model=BulkTelemetryResponse)
def bulk_insert_telemetry(payload: BulkTelemetryRequest, db: Session = Depends(get_db)):
    if not payload.readings:
        raise HTTPException(status_code=400, detail="Readings payload cannot be empty")
    values = [r.model_dump() for r in payload.readings]
    stmt = insert(TelemetryReading).returning(TelemetryReading.id)
    result = db.execute(stmt, values)
    inserted_ids = [row[0] for row in result.fetchall()]
    db.commit()
    return BulkTelemetryResponse(inserted_count=len(inserted_ids), ids=inserted_ids)
