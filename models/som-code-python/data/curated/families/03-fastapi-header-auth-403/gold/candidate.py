"""Multi-tenant document service that refuses reads across tenants."""

from collections.abc import Generator

from fastapi import Depends, FastAPI, Header, HTTPException, status
from pydantic import BaseModel, ConfigDict
from sqlalchemy import create_engine, select
from sqlalchemy.orm import DeclarativeBase, Mapped, Session, mapped_column, sessionmaker
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)
session_factory = sessionmaker(autocommit=False, autoflush=False, bind=engine)


class Base(DeclarativeBase):
    """Declarative base shared by every table in this module."""


class Document(Base):
    """A document owned by exactly one tenant."""

    __tablename__ = "documents"

    id: Mapped[int] = mapped_column(primary_key=True, index=True, autoincrement=True)
    tenant_id: Mapped[str] = mapped_column(nullable=False, index=True)
    title: Mapped[str] = mapped_column(nullable=False)
    content: Mapped[str] = mapped_column(nullable=False)


Base.metadata.create_all(bind=engine)


def seed_database() -> None:
    """Insert one document for each of two tenants when the table is empty."""
    db = session_factory()
    try:
        if db.scalar(select(Document).where(Document.id == 1)) is None:
            db.add(
                Document(
                    id=1,
                    tenant_id="tenant-alpha",
                    title="Alpha Strategy",
                    content="Classified Alpha",
                )
            )
            db.add(
                Document(
                    id=2,
                    tenant_id="tenant-beta",
                    title="Beta Plans",
                    content="Classified Beta",
                )
            )
            db.commit()
    finally:
        db.close()


seed_database()


def reset_db() -> None:
    """Recreate every table and seed it again."""
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    seed_database()


def get_db() -> Generator[Session, None, None]:
    """Yield one database session per request and close it afterwards."""
    db = session_factory()
    try:
        yield db
    finally:
        db.close()


class DocumentOut(BaseModel):
    """Document representation returned to clients."""

    id: int
    tenant_id: str
    title: str
    content: str

    model_config = ConfigDict(from_attributes=True)


app = FastAPI(title="Multi-Tenant Document Service")


@app.get("/documents/{doc_id}", response_model=DocumentOut)
def get_document(
    doc_id: int,
    x_tenant_id: str = Header(..., alias="X-Tenant-ID"),
    db: Session = Depends(get_db),
) -> Document:
    """Return a document to its own tenant: 404 when missing, 403 across tenants."""
    stmt = select(Document).where(Document.id == doc_id)
    doc = db.scalar(stmt)
    if doc is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Document not found",
        )
    if doc.tenant_id != x_tenant_id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Forbidden: Tenant mismatch",
        )
    return doc
