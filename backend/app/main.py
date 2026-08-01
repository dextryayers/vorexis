"""AIPentest backend: FastAPI orchestrator.

Flow:  Frontend → REST/WS → FastAPI → Rust engine (scan) → results → AI analysis → UI
"""
import asyncio
import logging
from contextlib import asynccontextmanager

from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware

from app.config import get_settings
from app.core.database import init_db, new_id
from app.core.errors import register_exception_handlers
from app.core.security import decode_token
from app.routers import auth, chat, scans, targets
from app.services import engine as engine_service

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(name)s %(levelname)s %(message)s")
log = logging.getLogger("app")


@asynccontextmanager
async def lifespan(app: FastAPI):
    settings = get_settings()
    await init_db(settings.database_url)
    log.info("database ready at %s", settings.database_url)
    log.info("engine binary: %s", settings.engine_binary)
    if settings.secret_key.startswith("aipentest-dev-secret"):
        log.warning("SECRET_KEY is the default dev value — set SECRET_KEY in .env for production")
    # Recover scans orphaned by a previous crash/restart.
    await engine_service.recover_stuck_scans()
    yield


app = FastAPI(title="AIPentest API", version="0.1.0", lifespan=lifespan)

settings = get_settings()
app.add_middleware(
    CORSMiddleware,
    allow_origins=[o.strip() for o in settings.cors_origins.split(",") if o.strip()],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

register_exception_handlers(app)

app.include_router(auth.router)
app.include_router(targets.router)
app.include_router(scans.router)
app.include_router(chat.router)


@app.get("/api/health")
async def health():
    from app.core.database import fetch_one

    try:
        await fetch_one("SELECT 1")
        db = "ok"
    except Exception:  # noqa: BLE001
        db = "error"
    return {"status": "ok", "app": settings.app_name, "engine": settings.engine_binary, "db": db}


@app.get("/api/modules")
async def modules():
    return engine_service.MODULES_META


@app.websocket("/ws/scan/{scan_id}")
async def ws_scan(websocket: WebSocket, scan_id: str):
    # Auth via `?token=` query param (browsers can't set WS headers).
    token = websocket.query_params.get("token")
    payload = decode_token(token) if token else None
    if not payload:
        await websocket.close(code=4401, reason="unauthorized")
        return

    await websocket.accept()
    active = engine_service._active_scans.get(scan_id)
    if not active or active.user_id != payload.get("sub"):
        await websocket.send_json({"type": "error", "message": "scan not active"})
        await websocket.close()
        return

    queue = active.subscribe()

    async def sender() -> None:
        while True:
            event = await queue.get()
            await websocket.send_json(event)

    async def receiver() -> None:
        # Discard client messages; raises on disconnect so gather() unwinds.
        while True:
            await websocket.receive_text()

    tasks = [asyncio.create_task(sender()), asyncio.create_task(receiver())]
    try:
        await asyncio.gather(*tasks)
    except WebSocketDisconnect:
        pass
    except Exception as exc:  # noqa: BLE001
        log.debug("ws error for scan %s: %s", scan_id, exc)
    finally:
        for t in tasks:
            t.cancel()
        active.unsubscribe(queue)
