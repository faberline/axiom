import uuid
from typing import Any, Dict, Optional
from fastapi import FastAPI, Request, status
from fastapi.responses import JSONResponse
from pydantic import BaseModel, Field


class TransferRequest(BaseModel):
    from_account: str = Field(min_length=1)
    to_account: str = Field(min_length=1)
    amount: float = Field(gt=0.0)


class DomainException(Exception):
    def __init__(self, message: str, code: str = "DOMAIN_ERROR", details: Optional[Dict[str, Any]] = None):
        super().__init__(message)
        self.message = message
        self.code = code
        self.details = details if details is not None else {}


class InsufficientFundsException(DomainException):
    def __init__(self, message: str = "Insufficient funds for transfer", details: Optional[Dict[str, Any]] = None):
        super().__init__(message=message, code="INSUFFICIENT_FUNDS", details=details)


class AccountNotFoundException(DomainException):
    def __init__(self, message: str = "Account not found", details: Optional[Dict[str, Any]] = None):
        super().__init__(message=message, code="ACCOUNT_NOT_FOUND", details=details)


ACCOUNTS_DB: Dict[str, float] = {
    "acc_1": 100.0,
    "acc_2": 50.0,
    "acc_3": 0.0,
}


def reset_db() -> None:
    global ACCOUNTS_DB
    ACCOUNTS_DB = {
        "acc_1": 100.0,
        "acc_2": 50.0,
        "acc_3": 0.0,
    }


app = FastAPI(title="Transfer Service")

@app.exception_handler(InsufficientFundsException)
async def insufficient_funds_handler(request: Request, exc: InsufficientFundsException):
    req_id = request.headers.get("X-Request-ID") or str(uuid.uuid4())
    return JSONResponse(
        status_code=status.HTTP_402_PAYMENT_REQUIRED,
        content={
            "error": {
                "code": exc.code,
                "message": exc.message,
                "details": exc.details,
                "request_id": req_id,
            }
        },
        headers={"X-Request-ID": req_id},
    )


@app.exception_handler(AccountNotFoundException)
async def account_not_found_handler(request: Request, exc: AccountNotFoundException):
    req_id = request.headers.get("X-Request-ID") or str(uuid.uuid4())
    return JSONResponse(
        status_code=status.HTTP_404_NOT_FOUND,
        content={
            "error": {
                "code": exc.code,
                "message": exc.message,
                "details": exc.details,
                "request_id": req_id,
            }
        },
        headers={"X-Request-ID": req_id},
    )


@app.post("/transfers", status_code=status.HTTP_200_OK)
async def create_transfer(payload: TransferRequest, request: Request):
    if payload.from_account not in ACCOUNTS_DB:
        raise AccountNotFoundException(
            message=f"Account '{payload.from_account}' not found",
            details={"account_id": payload.from_account},
        )
    if payload.to_account not in ACCOUNTS_DB:
        raise AccountNotFoundException(
            message=f"Account '{payload.to_account}' not found",
            details={"account_id": payload.to_account},
        )

    current_balance = ACCOUNTS_DB[payload.from_account]
    if current_balance < payload.amount:
        raise InsufficientFundsException(
            message=f"Account '{payload.from_account}' has insufficient balance",
            details={
                "available": current_balance,
                "requested": payload.amount,
                "shortfall": payload.amount - current_balance,
            },
        )

    ACCOUNTS_DB[payload.from_account] -= payload.amount
    ACCOUNTS_DB[payload.to_account] += payload.amount

    req_id = request.headers.get("X-Request-ID") or str(uuid.uuid4())
    return {
        "status": "completed",
        "from_account": payload.from_account,
        "to_account": payload.to_account,
        "amount": payload.amount,
        "remaining_balance": ACCOUNTS_DB[payload.from_account],
        "request_id": req_id,
    }

@app.get("/accounts/{account_id}")
async def get_account(account_id: str):
    if account_id not in ACCOUNTS_DB:
        raise AccountNotFoundException(
            message=f"Account '{account_id}' not found",
            details={"account_id": account_id},
        )
    return {"account_id": account_id, "balance": ACCOUNTS_DB[account_id]}
