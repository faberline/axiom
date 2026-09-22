from typing import Generator
from fastapi import Depends, FastAPI, Header, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import Column, Integer, String, create_engine, select
from sqlalchemy.orm import DeclarativeBase, Session, sessionmaker
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)


class Base(DeclarativeBase):
    pass


class Document(Base):
    __tablename__ = "documents"

    id = Column(Integer, primary_key=True, index=True, autoincrement=True)
    tenant_id = Column(String, nullable=False, index=True)
    title = Column(String, nullable=False)
    content = Column(String, nullable=False)


Base.metadata.create_all(bind=engine)


def seed_database() -> None:
    db = SessionLocal()
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
    Base.metadata.drop_all(bind=engine)
    Base.metadata.create_all(bind=engine)
    seed_database()


def get_db() -> Generator[Session, None, None]:
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()


class DocumentOut(BaseModel):
    id: int
    tenant_id: str
    title: str
    content: str

    class Config:
        from_attributes = True


app = FastAPI(title="Multi-Tenant Document Service")


@app.get("/documents/{doc_id}", response_model=DocumentOut)
def get_document(
    doc_id: int,
    x_tenant_id: str = Header(..., alias="X-Tenant-ID"),
    db: Session = Depends(get_db),
):
    stmt = select(Document).where(Document.id == doc_id)
    doc = db.scalar(stmt)
    if doc is None:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Document not found",
        )
    if doc.tenant_id != "admin":
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Forbidden: Tenant mismatch",
        )
    return doc
