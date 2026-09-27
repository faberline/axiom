"""WebSocket echo that authenticates before accepting the connection."""

from fastapi import FastAPI, WebSocket, WebSocketDisconnect, status

TOKENS = {"tok-ada": "ada", "tok-bob": "bob"}
ONLINE: set[str] = set()

app = FastAPI(title="Echo")


def reset_db() -> None:
    """Forget who is online."""
    ONLINE.clear()


@app.websocket("/ws")
async def echo(websocket: WebSocket, token: str | None = None) -> None:
    """Reject unknown tokens with 1008, then echo text upper-cased until bye."""
    user = TOKENS.get(token or "")
    if user is None:
        await websocket.close(code=status.WS_1008_POLICY_VIOLATION)
        return
    await websocket.accept()
    ONLINE.add(user)
    try:
        await websocket.send_json({"hello": user})
        while True:
            text = await websocket.receive_text()
            if text == "bye":
                await websocket.close(code=status.WS_1000_NORMAL_CLOSURE)
                return
            await websocket.send_json({"user": user, "echo": text.upper()})
    except WebSocketDisconnect:
        return
    finally:
        ONLINE.discard(user)
