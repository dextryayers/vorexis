import asyncio
import json

from fastapi import APIRouter, Depends, HTTPException
from fastapi.responses import StreamingResponse

from app.core.database import execute, fetch_all, fetch_one, new_id, now_iso
from app.core.deps import get_current_user
from app.models.schemas import ChatCreate, ChatMessageIn
from app.services.ai.manager import manager as ai_manager
from app.services.engine import MODULES_META, get_scan

router = APIRouter(prefix="/api/chat", tags=["chat"])

MAX_CONTEXT_MESSAGES = 40
MAX_CONTEXT_CHARS = 12_000


@router.get("/modules")
async def module_meta():
    return MODULES_META


@router.get("")
async def list_chats(user_id: str = Depends(get_current_user)):
    return await fetch_all(
        "SELECT id, title, scan_id, created_at, updated_at FROM chats WHERE user_id=? "
        "ORDER BY updated_at DESC LIMIT 100",
        (user_id,),
    )


@router.post("")
async def create_chat(body: ChatCreate, user_id: str = Depends(get_current_user)):
    chat_id = new_id()
    await execute(
        "INSERT INTO chats (id, user_id, scan_id, title, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        (chat_id, user_id, body.scan_id, body.title, now_iso(), now_iso()),
    )
    return {"id": chat_id, "title": body.title, "scan_id": body.scan_id}


@router.get("/{chat_id}/messages")
async def chat_messages(chat_id: str, user_id: str = Depends(get_current_user)):
    chat = await fetch_one("SELECT id, user_id FROM chats WHERE id=?", (chat_id,))
    if not chat or chat["user_id"] != user_id:
        raise HTTPException(404, "chat not found")
    return await fetch_all(
        "SELECT role, content, created_at FROM chat_messages WHERE chat_id=? ORDER BY id",
        (chat_id,),
    )


@router.delete("/{chat_id}")
async def delete_chat(chat_id: str, user_id: str = Depends(get_current_user)):
    chat = await fetch_one("SELECT id, user_id FROM chats WHERE id=?", (chat_id,))
    if not chat or chat["user_id"] != user_id:
        raise HTTPException(404, "chat not found")
    await execute("DELETE FROM chat_messages WHERE chat_id=?", (chat_id,))
    await execute("DELETE FROM chats WHERE id=?", (chat_id,))
    return {"ok": True}


async def _build_messages(chat_id: str, user_id: str) -> tuple[list[dict], str | None]:
    """Load the most recent conversation + optional scan context.

    Returns (message list newest-last, system prompt or None).
    """
    chat = await fetch_one("SELECT id, user_id, scan_id FROM chats WHERE id=?", (chat_id,))
    if not chat or chat["user_id"] != user_id:
        raise HTTPException(404, "chat not found")

    # Latest messages only (older ones beyond the window are excluded).
    rows = await fetch_all(
        """SELECT role, content FROM (
               SELECT id, role, content FROM chat_messages WHERE chat_id=?
               ORDER BY id DESC LIMIT ?
           ) ORDER BY id ASC""",
        (chat_id, MAX_CONTEXT_MESSAGES),
    )

    system = None
    if chat["scan_id"]:
        scan = await get_scan(chat["scan_id"], user_id)
        if scan and scan.get("summary"):
            context = ai_manager.build_scan_context(scan["summary"], scan.get("target"))
            system = (
                "You are AIPentest, an expert web security analyst. "
                f"Use this scan data when answering:\n{context}"
            )

    messages = [{"role": m["role"], "content": m["content"]} for m in rows]
    # Guard against context-window blowup from giant messages.
    total = sum(len(m["content"]) for m in messages)
    if total > MAX_CONTEXT_CHARS:
        trimmed: list[dict] = []
        budget = MAX_CONTEXT_CHARS
        for m in reversed(messages):
            content = m["content"]
            if len(content) > budget:
                content = content[: budget // 2] + "…"
            if budget <= 0:
                break
            budget -= len(content)
            trimmed.append({"role": m["role"], "content": content})
        messages = list(reversed(trimmed))
    return messages, system


async def _store_answer(chat_id: str, answer: str) -> None:
    await execute(
        "INSERT INTO chat_messages (chat_id, role, content, created_at) VALUES (?, 'assistant', ?, ?)",
        (chat_id, answer, now_iso()),
    )
    await execute("UPDATE chats SET updated_at=? WHERE id=?", (now_iso(), chat_id))


@router.post("/send")
async def send_message(body: ChatMessageIn, user_id: str = Depends(get_current_user)):
    await execute(
        "INSERT INTO chat_messages (chat_id, role, content, created_at) VALUES (?, 'user', ?, ?)",
        (body.chat_id, body.message, now_iso()),
    )
    messages, system = await _build_messages(body.chat_id, user_id)
    try:
        answer = await ai_manager.chat(messages, system=system)
    except RuntimeError as exc:
        # Don't persist a broken assistant turn — remove the user message.
        await execute(
            "DELETE FROM chat_messages WHERE chat_id=? AND role='user' AND content=?",
            (body.chat_id, body.message),
        )
        raise HTTPException(502, str(exc)) from exc
    await _store_answer(body.chat_id, answer)
    return {"role": "assistant", "content": answer}


@router.post("/send/stream")
async def send_message_stream(body: ChatMessageIn, user_id: str = Depends(get_current_user)):
    """SSE streaming chat. Persists the message + final answer at the end."""
    await execute(
        "INSERT INTO chat_messages (chat_id, role, content, created_at) VALUES (?, 'user', ?, ?)",
        (body.chat_id, body.message, now_iso()),
    )
    messages, system = await _build_messages(body.chat_id, user_id)

    async def gen():
        chunks: list[str] = []
        try:
            async for chunk in ai_manager.chat_stream(messages, system=system):
                chunks.append(chunk)
                yield f"data: {json.dumps({'delta': chunk})}\n\n"
            answer = "".join(chunks)
            await _store_answer(body.chat_id, answer)
            yield f"data: {json.dumps({'done': True})}\n\n"
        except asyncio.CancelledError:
            # Client disconnected mid-stream (user pressed stop) — persist the
            # partial answer so the conversation stays consistent.
            partial = "".join(chunks)
            if partial.strip():
                await _store_answer(body.chat_id, partial)
            raise
        except Exception as exc:  # noqa: BLE001
            yield f"data: {json.dumps({'error': str(exc)})}\n\n"
            return

    return StreamingResponse(
        gen(),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )
