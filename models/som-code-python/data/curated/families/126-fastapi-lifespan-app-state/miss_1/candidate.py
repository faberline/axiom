"""Service that opens its connection pool in lifespan and closes it on shutdown."""

from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from typing import Annotated

from fastapi import Depends, FastAPI, Request


class Pool:
    """A stand-in connection pool that refuses work once closed."""

    def __init__(self, size: int) -> None:
        if size < 1:
            raise ValueError("pool size must be at least 1")
        self.size = size
        self.closed = False
        self.queries = 0

    def query(self, sql: str) -> int:
        """Run a statement and return how many have run on this pool."""
        if self.closed:
            raise RuntimeError(f"pool is closed: cannot run {sql!r}")
        self.queries += 1
        return self.queries

    def close(self) -> None:
        """Release every connection."""
        self.closed = True


POOLS: list[Pool] = []


@asynccontextmanager
async def lifespan(application: FastAPI) -> AsyncIterator[None]:
    """Open one pool for the application's lifetime and always close it."""
    pool = Pool(size=5)
    POOLS.append(pool)
    application.state.pool = pool
    yield


app = FastAPI(title="Pooled service", lifespan=lifespan)


def get_pool(request: Request) -> Pool:
    """Return the pool opened by the running application's lifespan."""
    pool: Pool = request.app.state.pool
    return pool


PoolDep = Annotated[Pool, Depends(get_pool)]


@app.get("/count")
def count(pool: PoolDep) -> dict[str, int]:
    """Run a query and report the pool's running total."""
    return {"n": pool.query("select 1"), "size": pool.size}


@app.get("/health")
def health(pool: PoolDep) -> dict[str, str | bool]:
    """Report whether the pool is usable."""
    return {"status": "ok", "pool_closed": pool.closed}
