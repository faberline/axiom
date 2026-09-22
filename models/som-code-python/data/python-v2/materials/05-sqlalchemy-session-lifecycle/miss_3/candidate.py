from fastapi import FastAPI, HTTPException, Depends
from pydantic import BaseModel
from sqlalchemy import create_engine, Column, Integer, String
from sqlalchemy.orm import DeclarativeBase, sessionmaker, Session
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)


class Base(DeclarativeBase):
    pass


class Item(Base):
    __tablename__ = "items"

    id = Column(Integer, primary_key=True, index=True)
    name = Column(String, nullable=False)


Base.metadata.create_all(bind=engine)


class SessionMetrics:
    created: int = 0
    closed: int = 0


def reset_db() -> None:
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    SessionMetrics.created = 0
    SessionMetrics.closed = 0


class TrackedSession(Session):
    def close(self):
        SessionMetrics.closed += 1
        super().close()


SessionLocal = sessionmaker(
    class_=TrackedSession,
    autocommit=False,
    autoflush=False,
    bind=engine,
)

_shared_db = SessionLocal()


def get_db():
    SessionMetrics.created += 1
    try:
        yield _shared_db
    finally:
        pass


class ItemCreate(BaseModel):
    name: str


class ItemResponse(BaseModel):
    id: int
    name: str

    class Config:
        from_attributes = True


app = FastAPI()


@app.get("/session-metrics")
def get_session_metrics():
    return {
        "created": SessionMetrics.created,
        "closed": SessionMetrics.closed,
    }


@app.post("/items", status_code=201, response_model=ItemResponse)
def create_item(payload: ItemCreate, db: Session = Depends(get_db)):
    item = Item(name=payload.name)
    db.add(item)
    db.commit()
    db.refresh(item)
    return item


@app.get("/items", response_model=list[ItemResponse])
def list_items(db: Session = Depends(get_db)):
    return db.query(Item).all()


@app.get("/error-http")
def trigger_http_error(db: Session = Depends(get_db)):
    raise HTTPException(status_code=400, detail="Custom HTTP Error")


@app.post("/uncommitted-leak")
def uncommitted_leak(db: Session = Depends(get_db)):
    item = Item(name="Leaked Item")
    db.add(item)
    raise HTTPException(status_code=400, detail="Aborted before commit")


@app.post("/items-validated", status_code=201, response_model=ItemResponse)
def create_item_validated(payload: ItemCreate, db: Session = Depends(get_db)):
    if payload.name == "FAIL_TRIGGER":
        raise HTTPException(status_code=422, detail="Invalid item name")
    item = Item(name=payload.name)
    db.add(item)
    db.commit()
    db.refresh(item)
    return item
