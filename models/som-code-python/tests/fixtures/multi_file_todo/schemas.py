from __future__ import annotations

from datetime import datetime

from pydantic import BaseModel, ConfigDict


class TodoCreate(BaseModel):
    title: str
    description: str | None = None


class TodoUpdate(BaseModel):
    title: str | None = None
    description: str | None = None
    completed: bool | None = None


class TodoResponse(BaseModel):
    id: int
    title: str
    description: str | None = None
    completed: bool
    created_at: datetime | None = None

    model_config = ConfigDict(from_attributes=True)
