import json

from fastapi import APIRouter, Depends, HTTPException, Request

from app.core.database import deserialize, execute, fetch_all, fetch_one, new_id, now_iso
from app.core.deps import get_current_user
from app.core.ratelimit import rate_limited
from app.models.schemas import ScanCreate
from app.services import engine as engine_service
from app.services.ai.manager import manager as ai_manager

router = APIRouter(prefix="/api/scans", tags=["scans"])


@router.get("")
async def list_scans(user_id: str = Depends(get_current_user)):
    rows = await fetch_all(
        "SELECT id, target, modules, status, progress, started_at, finished_at, summary, error "
        "FROM scans WHERE user_id=? ORDER BY started_at DESC LIMIT 100",
        (user_id,),
    )
    for r in rows:
        r["modules"] = json.loads(r.get("modules") or "[]")
        r["summary"] = deserialize(r.get("summary"))
    return rows


@router.post("")
async def start_scan(body: ScanCreate, user_id: str = Depends(get_current_user), request: Request = None):
    if request is not None:
        if rate_limited(f"scan:{user_id}", max_per_minute=30):
            raise HTTPException(429, "too many scans — try again later")
    try:
        return await engine_service.start_scan(user_id, body.model_dump())
    except FileNotFoundError:
        raise HTTPException(
            500,
            "Rust engine binary not found — build it first: cd engine && cargo build --release",
        ) from None
    except RuntimeError as exc:
        raise HTTPException(429, str(exc)) from None
    except ValueError as exc:
        raise HTTPException(500, str(exc)) from None


@router.get("/{scan_id}")
async def get_scan(scan_id: str, user_id: str = Depends(get_current_user)):
    scan = await engine_service.get_scan(scan_id, user_id)
    if not scan:
        raise HTTPException(404, "scan not found")
    return scan


@router.get("/{scan_id}/events")
async def get_events(scan_id: str, user_id: str = Depends(get_current_user), limit: int = 2000):
    scan = await engine_service.get_scan(scan_id, user_id)
    if not scan:
        raise HTTPException(404, "scan not found")
    return await engine_service.get_scan_events(scan_id, user_id, min(max(limit, 1), 5000))


@router.post("/{scan_id}/stop")
async def stop_scan(scan_id: str, user_id: str = Depends(get_current_user)):
    ok = await engine_service.stop_scan(scan_id, user_id)
    if not ok:
        raise HTTPException(404, "scan not running")
    return {"ok": True}


@router.post("/{scan_id}/report")
async def generate_report(scan_id: str, user_id: str = Depends(get_current_user)):
    """AI-generated security report for a finished scan."""
    scan = await engine_service.get_scan(scan_id, user_id)
    if not scan:
        raise HTTPException(404, "scan not found")
    context = ai_manager.build_scan_context(scan.get("summary"), scan.get("target"))
    prompt = (
        "Write a professional penetration test report based on the scan data below. "
        "Structure it as: 1) Executive Summary 2) Findings table (severity / finding / impact / remediation) "
        "3) Detailed findings 4) Recommendations.\n\n" + context
    )
    try:
        report = await ai_manager.chat(
            [{"role": "user", "content": prompt}],
            system="You are a professional penetration testing report writer.",
        )
    except RuntimeError as exc:
        raise HTTPException(502, str(exc)) from exc
    return {"report": report}
