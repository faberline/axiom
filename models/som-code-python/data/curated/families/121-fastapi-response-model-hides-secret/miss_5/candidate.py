"""User signup API whose responses never expose the stored password hash."""

import hashlib
import secrets

from fastapi import FastAPI, HTTPException, status
from pydantic import BaseModel, Field


class UserIn(BaseModel):
    """Signup payload."""

    email: str = Field(min_length=3, max_length=254)
    password: str = Field(min_length=6)


class UserRecord(BaseModel):
    """Stored user, including the salted password hash."""

    id: int
    email: str
    password_hash: str


class UserOut(BaseModel):
    """Public user representation."""

    id: int
    email: str


USERS: dict[int, UserRecord] = {}
app = FastAPI(title="Accounts")


def reset_db() -> None:
    """Forget every stored user."""
    USERS.clear()


def hash_password(password: str, salt: str) -> str:
    """Return salt and scrypt digest joined by a dollar sign."""
    digest = hashlib.scrypt(password.encode(), salt=salt.encode(), n=2**14, r=8, p=1)
    return f"{salt}${digest.hex()}"


@app.post("/users", response_model=UserOut, status_code=status.HTTP_201_CREATED)
def create_user(body: UserIn) -> UserRecord:
    """Register a user under a normalized email address."""
    email = body.email.strip().lower()
    if any(user.email == email for user in USERS.values()):
        raise HTTPException(status.HTTP_409_CONFLICT, "email already registered")
    record = UserRecord(
        id=len(USERS) + 1,
        email=email,
        password_hash=hash_password(body.password, secrets.token_hex(8)),
    )
    USERS[record.id] = record
    return record


@app.get("/users/{user_id}", response_model=UserOut)
def get_user(user_id: int) -> UserRecord:
    """Return one user's public fields."""
    record = USERS.get(user_id)
    if record is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "user not found")
    return record
