from __future__ import annotations

from contextlib import asynccontextmanager

import uvicorn
from database import get_db, init_db
from fastapi import Depends, FastAPI, HTTPException, status
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from models import Todo
from schemas import TodoCreate, TodoResponse, TodoUpdate


@asynccontextmanager
async def lifespan(app: FastAPI):
    await init_db()
    yield


app = FastAPI(title="Todo API", lifespan=lifespan)


@app.get("/api/todos", response_model=list[TodoResponse])
async def list_todos(db: AsyncSession = Depends(get_db)):  # noqa: B008
    result = await db.execute(select(Todo).order_by(Todo.created_at.desc()))
    return result.scalars().all()


@app.post("/api/todos", response_model=TodoResponse, status_code=status.HTTP_201_CREATED)
async def create_todo(todo: TodoCreate, db: AsyncSession = Depends(get_db)):  # noqa: B008
    item = Todo(
        title=todo.title,
        description=todo.description,
    )
    db.add(item)
    await db.commit()
    await db.refresh(item)
    return item


@app.get("/api/todos/{todo_id}", response_model=TodoResponse)
async def get_todo(todo_id: int, db: AsyncSession = Depends(get_db)):  # noqa: B008
    result = await db.execute(select(Todo).where(Todo.id == todo_id))
    item = result.scalar_one_or_none()
    if not item:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Todo not found")
    return item


@app.patch("/api/todos/{todo_id}", response_model=TodoResponse)
async def update_todo(todo_id: int, todo: TodoUpdate, db: AsyncSession = Depends(get_db)):  # noqa: B008
    result = await db.execute(select(Todo).where(Todo.id == todo_id))
    item = result.scalar_one_or_none()
    if not item:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Todo not found")
    if todo.title is not None:
        item.title = todo.title
    if todo.description is not None:
        item.description = todo.description
    if todo.completed is not None:
        item.completed = todo.completed
    await db.commit()
    await db.refresh(item)
    return item


@app.delete("/api/todos/{todo_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_todo(todo_id: int, db: AsyncSession = Depends(get_db)):  # noqa: B008
    result = await db.execute(select(Todo).where(Todo.id == todo_id))
    item = result.scalar_one_or_none()
    if not item:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Todo not found")
    await db.delete(item)
    await db.commit()


if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=8000, reload=False)
