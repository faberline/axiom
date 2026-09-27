"""A broadcast chat room over aiohttp.web WebSockets."""

from __future__ import annotations

from aiohttp import WSCloseCode, WSMsgType, web

MAX_MESSAGE_CHARS = 500
CLIENTS_KEY = web.AppKey("clients", set[web.WebSocketResponse])


async def chat(request: web.Request) -> web.WebSocketResponse:
    """Relay each text message to every other connected client."""
    ws = web.WebSocketResponse(heartbeat=30.0)
    await ws.prepare(request)
    clients = request.app[CLIENTS_KEY]
    clients.add(ws)
    try:
        async for msg in ws:
            if msg.type != WSMsgType.TEXT:
                await ws.close(code=WSCloseCode.UNSUPPORTED_DATA, message=b"text only")
                break
            if len(msg.data) >= MAX_MESSAGE_CHARS:
                await ws.send_json({"error": "message too long"})
                continue
            for peer in list(clients):
                if peer is not ws and not peer.closed:
                    await peer.send_json({"message": msg.data})
    finally:
        clients.discard(ws)
    return ws


async def _close_clients(app: web.Application) -> None:
    for ws in app[CLIENTS_KEY].copy():
        await ws.close(code=WSCloseCode.GOING_AWAY, message=b"server shutdown")


def make_app() -> web.Application:
    """Build the chat application with an empty client registry."""
    app = web.Application()
    app[CLIENTS_KEY] = set()
    app.router.add_get("/ws", chat)
    app.on_shutdown.append(_close_clients)
    return app
