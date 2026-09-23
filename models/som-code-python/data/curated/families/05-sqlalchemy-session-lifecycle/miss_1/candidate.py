"""Item service that counts the sessions it opens and closes, to prove none leak."""

from collections.abc import Generator

from fastapi import Depends, FastAPI, HTTPException
from pydantic import BaseModel, ConfigDict
from sqlalchemy import create_engine, select
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, sessionmaker
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)


class Base(DeclarativeBase):
    """Declarative base shared by every table in this module."""


class Item(Base):
    """A persisted catalog item."""

    __tablename__ = "items"

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    name: Mapped[str] = mapped_column(nullable=False)


Base.metadata.create_all(bind=engine)


class SessionMetrics:
    """Process-wide counters of sessions opened and closed."""

    created: int = 0
    closed: int = 0


def reset_db() -> None:
    """Recreate every table and zero the session counters."""
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    SessionMetrics.created = 0
    SessionMetrics.closed = 0


class TrackedSession(Session):
    """Session that counts every close."""

    def close(self) -> None:
        """Count this close, then close the session."""
        SessionMetrics.closed += 1
        super().close()


session_factory = sessionmaker(
    class_=TrackedSession,
    autocommit=False,
    autoflush=False,
    bind=engine,
)


def get_db() -> Generator[Session, None, None]:
    """Open one counted session per request and always close it."""
    SessionMetrics.created += 1
    db = session_factory()
    yield db
    db.close()


class ItemCreate(BaseModel):
    """Payload accepted when creating an item."""

    name: str


class ItemResponse(BaseModel):
    """Item representation returned to clients."""

    id: int
    name: str

    model_config = ConfigDict(from_attributes=True)


app = FastAPI()


@app.get("/session-metrics")
def get_session_metrics() -> dict[str, int]:
    """Report how many sessions were opened and closed."""
    return {
        "created": SessionMetrics.created,
        "closed": SessionMetrics.closed,
    }


@app.post("/items", status_code=201, response_model=ItemResponse)
def create_item(payload: ItemCreate, db: Session = Depends(get_db)) -> Item:
    """Persist a new item and return it."""
    item = Item(name=payload.name)
    db.add(item)
    db.commit()
    db.refresh(item)
    return item


@app.get("/items", response_model=list[ItemResponse])
def list_items(db: Session = Depends(get_db)) -> list[Item]:
    """Return every item."""
    return list(db.scalars(select(Item)))


@app.get("/error-http")
def trigger_http_error(_db: Session = Depends(get_db)) -> None:
    """Open a session, then fail with 400 before using it."""
    raise HTTPException(status_code=400, detail="Custom HTTP Error")


@app.post("/uncommitted-leak")
def uncommitted_leak(db: Session = Depends(get_db)) -> None:
    """Add an item, then abort with 400 before committing it."""
    item = Item(name="Leaked Item")
    db.add(item)
    raise HTTPException(status_code=400, detail="Aborted before commit")


@app.post("/items-validated", status_code=201, response_model=ItemResponse)
def create_item_validated(payload: ItemCreate, db: Session = Depends(get_db)) -> Item:
    """Refuse a reserved name with 422 before touching the session."""
    if payload.name == "FAIL_TRIGGER":
        raise HTTPException(status_code=422, detail="Invalid item name")
    item = Item(name=payload.name)
    db.add(item)
    db.commit()
    db.refresh(item)
    return item
