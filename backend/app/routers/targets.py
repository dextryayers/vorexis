from fastapi import APIRouter, Depends, HTTPException

from app.core.database import execute, fetch_all, fetch_one, new_id, now_iso
from app.core.deps import get_current_user
from app.models.schemas import TargetCreate, TargetResponse

router = APIRouter(prefix="/api/targets", tags=["targets"])


@router.get("")
async def list_targets(user_id: str = Depends(get_current_user)):
    rows = await fetch_all(
        "SELECT * FROM targets WHERE user_id=? ORDER BY created_at DESC", (user_id,)
    )
    return rows


@router.post("", response_model=TargetResponse)
async def create_target(body: TargetCreate, user_id: str = Depends(get_current_user)):
    tid = new_id()
    await execute(
        "INSERT INTO targets (id, user_id, url, label, created_at) VALUES (?, ?, ?, ?, ?)",
        (tid, user_id, body.url, body.label, now_iso()),
    )
    return TargetResponse(id=tid, url=body.url, label=body.label, created_at=now_iso())


@router.delete("/{target_id}")
async def delete_target(target_id: str, user_id: str = Depends(get_current_user)):
    row = await fetch_one("SELECT id FROM targets WHERE id=? AND user_id=?", (target_id, user_id))
    if not row:
        raise HTTPException(404, "target not found")
    await execute("DELETE FROM targets WHERE id=?", (target_id,))
    return {"ok": True}
