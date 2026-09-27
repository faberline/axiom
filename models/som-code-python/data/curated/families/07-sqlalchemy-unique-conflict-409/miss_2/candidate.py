"""User service: register users and answer 409 when an email is already taken."""

from collections.abc import Generator

from fastapi import Depends, FastAPI, HTTPException
from pydantic import BaseModel, ConfigDict
from sqlalchemy import create_engine, select
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, sessionmaker
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)


class Base(DeclarativeBase):
    """Declarative base shared by every table in this module."""


class User(Base):
    """A registered user with a unique email."""

    __tablename__ = "users"

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    email: Mapped[str] = mapped_column(unique=True, nullable=False, index=True)
    username: Mapped[str] = mapped_column(nullable=False)


Base.metadata.create_all(bind=engine)


def reset_db() -> None:
    """Drop and recreate every table."""
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)


session_factory = sessionmaker(autocommit=False, autoflush=False, bind=engine)


def get_db() -> Generator[Session, None, None]:
    """Yield one database session per request and close it afterwards."""
    db = session_factory()
    try:
        yield db
    finally:
        db.close()


class UserCreate(BaseModel):
    """Payload accepted when registering a user."""

    email: str
    username: str


class UserResponse(BaseModel):
    """User representation returned to clients."""

    id: int
    email: str
    username: str

    model_config = ConfigDict(from_attributes=True)


app = FastAPI()


@app.get("/users", response_model=list[UserResponse])
def list_users(db: Session = Depends(get_db)) -> list[User]:
    """Return every user."""
    return list(db.scalars(select(User)))


@app.get("/users/{user_id}", response_model=UserResponse)
def get_user(user_id: int, db: Session = Depends(get_db)) -> User:
    """Return one user by id, or 404 when it does not exist."""
    user = db.get(User, user_id)
    if not user:
        raise HTTPException(status_code=404, detail="User not found")
    return user


@app.post("/users", status_code=201, response_model=UserResponse)
def create_user(payload: UserCreate, db: Session = Depends(get_db)) -> User:
    """Persist a new user, or answer 409 when the email is already registered."""
    user = User(email=payload.email, username=payload.username)
    db.add(user)
    try:
        db.commit()
        db.refresh(user)
        return user
    except IntegrityError as exc:
        db.rollback()
        raise HTTPException(status_code=400, detail="Email already registered") from exc
