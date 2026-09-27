"""Order service: place multi-item orders atomically, rolling back on any failure."""

from __future__ import annotations

from collections.abc import Generator

from fastapi import Depends, FastAPI, HTTPException
from pydantic import BaseModel, ConfigDict
from sqlalchemy import ForeignKey, create_engine, select
from sqlalchemy.orm import (
    DeclarativeBase,
    Mapped,
    Session,
    mapped_column,
    relationship,
    sessionmaker,
)
from sqlalchemy.pool import StaticPool

engine = create_engine(
    "sqlite:///:memory:",
    connect_args={"check_same_thread": False},
    poolclass=StaticPool,
)


class Base(DeclarativeBase):
    """Declarative base shared by every table in this module."""


class Product(Base):
    """A product with a price and a stock count."""

    __tablename__ = "products"

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    name: Mapped[str] = mapped_column(nullable=False)
    stock: Mapped[int] = mapped_column(nullable=False)
    price: Mapped[float] = mapped_column(nullable=False)


class Order(Base):
    """A placed order and its line items."""

    __tablename__ = "orders"

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    total_price: Mapped[float] = mapped_column(nullable=False, default=0.0)
    items: Mapped[list[OrderItem]] = relationship("OrderItem", back_populates="order")


class OrderItem(Base):
    """One product line of an order, priced at order time."""

    __tablename__ = "order_items"

    id: Mapped[int] = mapped_column(primary_key=True, index=True)
    order_id: Mapped[int] = mapped_column(ForeignKey("orders.id"), nullable=False)
    product_id: Mapped[int] = mapped_column(ForeignKey("products.id"), nullable=False)
    quantity: Mapped[int] = mapped_column(nullable=False)
    unit_price: Mapped[float] = mapped_column(nullable=False)

    order: Mapped[Order] = relationship("Order", back_populates="items")


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


class ProductCreate(BaseModel):
    """Payload accepted when creating a product."""

    name: str
    stock: int
    price: float


class ProductResponse(BaseModel):
    """Product representation returned to clients."""

    id: int
    name: str
    stock: int
    price: float

    model_config = ConfigDict(from_attributes=True)


class OrderItemRequest(BaseModel):
    """One requested product line of a new order."""

    product_id: int
    quantity: int


class OrderCreate(BaseModel):
    """Payload accepted when placing an order."""

    items: list[OrderItemRequest]


class OrderItemResponse(BaseModel):
    """Order line representation returned to clients."""

    product_id: int
    quantity: int
    unit_price: float

    model_config = ConfigDict(from_attributes=True)


class OrderResponse(BaseModel):
    """Order representation returned to clients."""

    id: int
    total_price: float
    items: list[OrderItemResponse]

    model_config = ConfigDict(from_attributes=True)


app = FastAPI()


@app.post("/products", status_code=201, response_model=ProductResponse)
def create_product(payload: ProductCreate, db: Session = Depends(get_db)) -> Product:
    """Persist a new product and return it."""
    product = Product(
        name=payload.name,
        stock=payload.stock,
        price=payload.price,
    )
    db.add(product)
    db.commit()
    db.refresh(product)
    return product


@app.get("/products/{product_id}", response_model=ProductResponse)
def get_product(product_id: int, db: Session = Depends(get_db)) -> Product:
    """Return one product by id, or 404 when it does not exist."""
    product = db.get(Product, product_id)
    if not product:
        raise HTTPException(status_code=404, detail="Product not found")
    return product


@app.get("/orders", response_model=list[OrderResponse])
def list_orders(db: Session = Depends(get_db)) -> list[Order]:
    """Return every order."""
    return list(db.scalars(select(Order)))


@app.get("/orders/{order_id}", response_model=OrderResponse)
def get_order(order_id: int, db: Session = Depends(get_db)) -> Order:
    """Return one order by id, or 404 when it does not exist."""
    order = db.get(Order, order_id)
    if not order:
        raise HTTPException(status_code=404, detail="Order not found")
    return order


@app.post("/orders", status_code=201, response_model=OrderResponse)
def create_order(payload: OrderCreate, db: Session = Depends(get_db)) -> Order:
    """Reserve stock for every line and commit the order as one transaction."""
    if not payload.items:
        raise HTTPException(
            status_code=400, detail="Order must contain at least one item"
        )

    order = Order(total_price=0.0)
    db.add(order)

    try:
        total = 0.0
        for item_req in payload.items:
            product = db.get(Product, item_req.product_id)
            if not product:
                raise HTTPException(
                    status_code=404, detail=f"Product {item_req.product_id} not found"
                )

            product.stock -= item_req.quantity
            item_total = product.price * item_req.quantity
            total += item_total

            order_item = OrderItem(
                order=order,
                product_id=product.id,
                quantity=item_req.quantity,
                unit_price=product.price,
            )
            db.add(order_item)

        order.total_price = total
        db.commit()
        db.refresh(order)
        return order
    except HTTPException:
        db.rollback()
        raise
    except Exception:
        db.rollback()
        raise
