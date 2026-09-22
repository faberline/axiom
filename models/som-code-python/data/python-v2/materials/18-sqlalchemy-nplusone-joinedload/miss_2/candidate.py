from fastapi import FastAPI, Depends
from pydantic import BaseModel
from sqlalchemy import create_engine, Column, Integer, String, ForeignKey, select, event
from sqlalchemy.orm import DeclarativeBase, sessionmaker, Session, relationship, joinedload
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)


class QueryCounter:
    count: int = 0
    statements: list[str] = []

    @classmethod
    def reset(cls):
        cls.count = 0
        cls.statements = []


@event.listens_for(engine, "before_cursor_execute")
def before_cursor_execute(conn, cursor, statement, parameters, context, executemany):
    if statement.strip().upper().startswith("SELECT"):
        QueryCounter.count += 1
        QueryCounter.statements.append(statement)


class Base(DeclarativeBase):
    pass


class Author(Base):
    __tablename__ = "authors"

    id = Column(Integer, primary_key=True, index=True)
    name = Column(String, nullable=False)
    email = Column(String, nullable=False)


class Article(Base):
    __tablename__ = "articles"

    id = Column(Integer, primary_key=True, index=True)
    title = Column(String, nullable=False)
    content = Column(String, nullable=False)
    author_id = Column(Integer, ForeignKey("authors.id"), nullable=True)

    author = relationship("Author", lazy="select")


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
    db = SessionLocal()
    a1 = Author(name="Alice Smith", email="alice@example.com")
    a2 = Author(name="Bob Jones", email="bob@example.com")
    db.add_all([a1, a2])
    db.flush()

    art1 = Article(title="Deep Dive into Async", content="Async content", author_id=a1.id)
    art2 = Article(title="SQLAlchemy Patterns", content="SQLAlchemy content", author_id=a1.id)
    art3 = Article(title="Distributed Systems", content="Systems content", author_id=a2.id)
    art4 = Article(title="Anonymous Editorial", content="Opinion content", author_id=None)
    db.add_all([art1, art2, art3, art4])
    db.commit()
    db.close()
    QueryCounter.reset()


reset_db()


class AuthorResponse(BaseModel):
    id: int
    name: str
    email: str

    class Config:
        from_attributes = True


class ArticleResponse(BaseModel):
    id: int
    title: str
    content: str
    author: AuthorResponse | None = None

    class Config:
        from_attributes = True


app = FastAPI()


@app.get("/query-metrics")
def get_query_metrics():
    return {"count": QueryCounter.count}


@app.get("/articles", response_model=list[ArticleResponse])
def get_articles(db: Session = Depends(get_db)):
    stmt = select(Article).options(joinedload(Article.author, innerjoin=True))
    articles = db.scalars(stmt).all()
    return articles
