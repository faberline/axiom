from fastapi import FastAPI, Depends
from pydantic import BaseModel
from sqlalchemy import create_engine, Column, Integer, String, Float, ForeignKey, select, event
from sqlalchemy.orm import DeclarativeBase, sessionmaker, Session, relationship, selectinload
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


class Order(Base):
    __tablename__ = "orders"

    id = Column(Integer, primary_key=True, index=True)
    order_number = Column(String, nullable=False)
    status = Column(String, default="completed", nullable=False)

    items = relationship("OrderItem", back_populates="order", lazy="select")


class OrderItem(Base):
    __tablename__ = "order_items"

    id = Column(Integer, primary_key=True, index=True)
    order_id = Column(Integer, ForeignKey("orders.id"), nullable=False)
    product_name = Column(String, nullable=False)
    quantity = Column(Integer, nullable=False)
    price = Column(Float, nullable=False)

    order = relationship("Order", back_populates="items")


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
    for i in range(1, 6):
        order = Order(order_number=f"ORD-{i:03d}", status="completed")
        db.add(order)
        db.flush()
        item1 = OrderItem(order_id=order.id, product_name=f"Item {i}-A", quantity=1, price=10.0)
        item2 = OrderItem(order_id=order.id, product_name=f"Item {i}-B", quantity=2, price=20.0)
        db.add_all([item1, item2])
    db.commit()
    db.close()
    QueryCounter.reset()


reset_db()


class OrderItemResponse(BaseModel):
    id: int
    product_name: str
    quantity: int
    price: float

    class Config:
        from_attributes = True


class OrderResponse(BaseModel):
    id: int
    order_number: str
    status: str
    items: list[OrderItemResponse]

    class Config:
        from_attributes = True


app = FastAPI()


@app.get("/query-metrics")
def get_query_metrics():
    return {"count": QueryCounter.count}


@app.get("/orders", response_model=list[OrderResponse])
def get_orders(include_details: bool = True, db: Session = Depends(get_db)):
    stmt = select(Order)
    if include_details:
        stmt = stmt.options(selectinload(Order.items))
    orders = db.scalars(stmt).all()
    return [
        OrderResponse(
            id=o.id,
            order_number=o.order_number,
            status=o.status,
            items=[],
        )
        for o in orders
    ]
