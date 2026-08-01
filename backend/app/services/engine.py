"""Orchestrates the Rust engine: spawns the binary, parses NDJSON,
persists events and broadcasts them over WebSockets."""
import asyncio
import json
import logging
import subprocess
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

from app.config import get_settings
from app.core.database import (
    deserialize,
    execute,
    execute_many,
    fetch_all,
    fetch_one,
    new_id,
    now_iso,
    serialize,
)

log = logging.getLogger("engine")

MODULES_META = {
    "port": {"label": "Port Scanner", "description": "TCP connect scan with banner grabbing"},
    "directory": {"label": "Directory Scanner", "description": "Brute-force paths and files"},
    "subdomain": {"label": "Subdomain Scanner", "description": "Enumerate subdomains via DNS"},
    "dns": {"label": "DNS", "description": "Query A/AAAA/CNAME/MX/NS/TXT/SOA records"},
    "crawler": {"label": "Crawler", "description": "BFS spider across same-host pages"},
    "parser": {"label": "Parser", "description": "HTML parsing: metadata, forms, scripts, comments"},
    "http": {"label": "HTTP", "description": "Header & security header analysis, cookies, methods"},
    "https": {"label": "HTTPS", "description": "HTTPS endpoint behavior analysis"},
    "tls": {"label": "SSL/TLS", "description": "Certificate, protocol & cipher suite inspection"},
    "ssl": {"label": "SSL", "description": "Alias for TLS analysis"},
    "fuzzer": {"label": "Fuzzer", "description": "URL variant & payload fuzzing"},
    "waf": {"label": "WAF Detection", "description": "Send attack probes, match WAF signatures"},
    "fingerprint": {"label": "Fingerprint", "description": "Server & software fingerprinting"},
    "tech": {"label": "Tech Detection", "description": "Detect frameworks, CMS, CDN, analytics"},
}

MAX_CONCURRENT_SCANS_PER_USER = 2
ENGINE_EVENT_FLUSH_SECONDS = 1.0


@dataclass
class ActiveScan:
    scan_id: str
    user_id: str
    process: subprocess.Popen
    stopped: bool = False
    subscribers: set[asyncio.Queue] = field(default_factory=set)

    def subscribe(self) -> asyncio.Queue:
        q: asyncio.Queue = asyncio.Queue(maxsize=500)
        self.subscribers.add(q)
        return q

    def unsubscribe(self, q: asyncio.Queue) -> None:
        self.subscribers.discard(q)

    async def broadcast(self, event: dict) -> None:
        dead = []
        for q in self.subscribers:
            try:
                q.put_nowait(event)
            except asyncio.QueueFull:
                dead.append(q)
        for q in dead:
            self.unsubscribe(q)


_active_scans: dict[str, ActiveScan] = {}
_user_scan_count: dict[str, int] = {}


def build_job(payload: dict) -> dict:
    modules = payload.get("modules") or ["http", "tls", "fingerprint", "tech", "waf"]
    concurrency = max(1, min(int(payload.get("concurrency", 50)), 200))
    timeout = max(1, min(int(payload.get("timeout", 8)), 30))
    return {
        "target": payload["target"],
        "modules": [m.lower() for m in modules],
        "concurrency": concurrency,
        "timeout": timeout,
        "options": payload.get("options", {}),
        "wordlists": payload.get("wordlists", {}),
    }


def can_start_scan(user_id: str) -> bool:
    return _user_scan_count.get(user_id, 0) < MAX_CONCURRENT_SCANS_PER_USER


async def start_scan(user_id: str, payload: dict) -> dict:
    if not can_start_scan(user_id):
        raise RuntimeError(
            f"you can run at most {MAX_CONCURRENT_SCANS_PER_USER} scans at once"
        )

    settings = get_settings()
    binary = settings.engine_binary
    if not binary or not Path(binary).is_file():
        raise FileNotFoundError(binary or "engine binary not configured")

    scan_id = new_id()
    target = payload["target"]
    modules = payload.get("modules") or ["http", "tls", "fingerprint", "tech", "waf"]
    options = payload.get("options", {})
    now = now_iso()

    await execute(
        """INSERT INTO scans (id, user_id, target, modules, options, status, started_at)
           VALUES (?, ?, ?, ?, ?, 'running', ?)""",
        (scan_id, user_id, target, json.dumps(modules), json.dumps(options), now),
    )

    job = build_job(payload)

    proc = await asyncio.to_thread(
        subprocess.Popen,
        [binary],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    proc.stdin.write(json.dumps(job))
    proc.stdin.flush()
    proc.stdin.close()

    active = ActiveScan(scan_id=scan_id, user_id=user_id, process=proc)
    _active_scans[scan_id] = active
    _user_scan_count[user_id] = _user_scan_count.get(user_id, 0) + 1

    asyncio.create_task(_drain(scan_id, proc, settings.engine_timeout))
    return {
        "id": scan_id,
        "target": target,
        "modules": modules,
        "status": "running",
        "started_at": now,
    }


async def _read_lines_into_queue(
    stream, line_queue: asyncio.Queue, source: str, proc: subprocess.Popen
) -> None:
    """Blocking reader thread bridge: per-scan thread, not per-line."""
    try:
        while True:
            line = await asyncio.to_thread(stream.readline)
            if not line:
                break
            try:
                line_queue.put_nowait(line)
            except asyncio.QueueFull:
                # Slow consumer: wait a little, then retry (backpressure).
                await asyncio.sleep(0.05)
                try:
                    line_queue.put_nowait(line)
                except asyncio.QueueFull:
                    break
    finally:
        try:
            proc.wait(timeout=1)
        except Exception:  # noqa: BLE001
            pass
        await line_queue.put(None)  # EOF marker (one per stream is fine)


async def _watchdog(scan_id: str, proc: subprocess.Popen, engine_timeout: int) -> None:
    await asyncio.sleep(engine_timeout)
    if proc.poll() is None:
        log.warning("scan %s exceeded %ss — killing engine", scan_id, engine_timeout)
        try:
            proc.kill()
        except Exception:  # noqa: BLE001
            pass
        await execute(
            """UPDATE scans SET status='failed', finished_at=?, error='scan timed out'
               WHERE id=? AND status='running'""",
            (now_iso(), scan_id),
        )


async def _drain(scan_id: str, proc: subprocess.Popen, engine_timeout: int) -> None:
    """Read engine NDJSON stdout and fan out to subscribers + DB."""
    active = _active_scans.get(scan_id)
    line_queue: asyncio.Queue = asyncio.Queue(maxsize=1000)
    reader_tasks = [
        asyncio.create_task(_read_lines_into_queue(proc.stdout, line_queue, "stdout", proc)),
        asyncio.create_task(_read_lines_into_queue(proc.stderr, line_queue, "stderr", proc)),
    ]
    watchdog = asyncio.create_task(_watchdog(scan_id, proc, engine_timeout))

    buffer: list[tuple] = []
    eof_count = 0
    last_flush = time.monotonic()
    max_progress = 0.0
    last_persisted_progress = 0.0
    results_seen = 0

    async def flush_events() -> None:
        nonlocal buffer
        if not buffer:
            return
        await execute_many(
            """INSERT INTO scan_events (scan_id, module, event_type, data, created_at)
               VALUES (?, ?, ?, ?, ?)""",
            buffer,
        )
        buffer = []

    async def persist_progress() -> None:
        nonlocal last_persisted_progress
        if max_progress > last_persisted_progress:
            await execute(
                "UPDATE scans SET progress=? WHERE id=?", (max_progress, scan_id)
            )
            last_persisted_progress = max_progress

    try:
        while eof_count < 2:
            try:
                raw = await asyncio.wait_for(line_queue.get(), timeout=1.0)
            except asyncio.TimeoutError:
                # Periodic flush + progress persistence.
                await flush_events()
                await persist_progress()
                continue
            if raw is None:
                eof_count += 1
                continue

            try:
                event = json.loads(raw)
            except json.JSONDecodeError:
                if raw.strip():
                    log.debug("non-JSON line from engine: %r", raw[:200])
                continue

            await active.broadcast(event)

            module = event.get("module", "")
            etype = event.get("type", "")
            data = event.get("data")
            if etype == "result":
                results_seen += 1
            if etype == "progress":
                total = event.get("total") or 0
                current = event.get("current") or 0
                if total:
                    max_progress = max(max_progress, round(current / total * 100, 1))
            buffer.append((scan_id, module, etype, serialize(data), now_iso()))
            if len(buffer) >= 50 or time.monotonic() - last_flush >= ENGINE_EVENT_FLUSH_SECONDS:
                await flush_events()
                last_flush = time.monotonic()

        await flush_events()
        await persist_progress()
    except asyncio.CancelledError:
        proc.terminate()
        await flush_events()
        raise
    except Exception as exc:  # noqa: BLE001
        log.error("drain error for scan %s: %s", scan_id, exc)
        proc.terminate()

    for t in reader_tasks:
        t.cancel()
    watchdog.cancel()

    try:
        await asyncio.to_thread(proc.wait, 5)
    except Exception:  # noqa: BLE001
        proc.kill()

    if active.stopped:
        status, error = "stopped", None
    elif proc.returncode != 0 and results_seen == 0:
        status, error = "failed", f"engine exited with code {proc.returncode}"
    else:
        status = "finished"
        error = None

    summary = await _build_summary(scan_id) if status == "finished" else None
    await execute(
        """UPDATE scans SET status=?, finished_at=?, summary=?, progress=?, error=?
           WHERE id=?""",
        (status, now_iso(), serialize(summary), 100.0 if status == "finished" else 0.0,
         error, scan_id),
    )
    await active.broadcast(
        {"type": "done", "module": "engine", "scan_id": scan_id, "summary": summary,
         "returncode": proc.returncode}
    )
    _active_scans.pop(scan_id, None)
    _user_scan_count[active.user_id] = max(0, _user_scan_count.get(active.user_id, 1) - 1)


async def _build_summary(scan_id: str) -> dict:
    rows = await fetch_all(
        "SELECT module, event_type, data FROM scan_events WHERE scan_id=? ORDER BY id", (scan_id,)
    )
    per_module: dict[str, dict] = {}
    for r in rows:
        m = r["module"]
        if m not in per_module:
            per_module[m] = {"module": m, "results": 0, "events": 0, "data": []}
        per_module[m]["events"] += 1
        if r["event_type"] == "result":
            per_module[m]["results"] += 1
            data = deserialize(r["data"])
            if isinstance(data, dict):
                per_module[m]["data"].append(data)

    modules = []
    for key, val in per_module.items():
        item_results = [d for d in val["data"] if "port" in d or "url" in d]
        aggregate = [
            d for d in val["data"]
            if "open_ports" in d or "found" in d or "records" in d or "technologies" in d
            or "detected_wafs" in d or "security_headers" in d or "pages_crawled" in d
            or "fingerprint" in d or "certificates" in d or "forms" in d
        ]
        modules.append({
            "module": key,
            "result_count": val["results"],
            "details": (aggregate or item_results)[-8:],
        })
    return {"modules": modules, "total_events": len(rows)}


async def stop_scan(scan_id: str, user_id: str) -> bool:
    active = _active_scans.get(scan_id)
    if not active or active.user_id != user_id:
        return False
    active.stopped = True
    try:
        active.process.terminate()
    except Exception:  # noqa: BLE001
        pass
    return True


async def recover_stuck_scans() -> None:
    """Mark scans orphaned by a previous crash as failed."""
    rows = await fetch_all("SELECT id FROM scans WHERE status IN ('running','pending')")
    for r in rows:
        await execute(
            """UPDATE scans SET status='failed', finished_at=?, error='engine restarted'
               WHERE id=?""",
            (now_iso(), r["id"]),
        )
        log.info("recovered stuck scan %s -> failed", r["id"])


async def get_scan(scan_id: str, user_id: str) -> dict | None:
    row = await fetch_one(
        "SELECT * FROM scans WHERE id=? AND user_id=?", (scan_id, user_id)
    )
    if not row:
        return None
    row["modules"] = json.loads(row["modules"]) if row.get("modules") else []
    row["summary"] = deserialize(row.get("summary"))
    return row


async def get_scan_events(scan_id: str, user_id: str, limit: int = 2000) -> list[dict]:
    limit = max(1, min(limit, 5000))
    rows = await fetch_all(
        """SELECT e.* FROM (
               SELECT * FROM scan_events WHERE scan_id=?
               ORDER BY id DESC LIMIT ?
           ) e
           JOIN scans s ON s.id = e.scan_id
           WHERE s.user_id=?
           ORDER BY e.id ASC""",
        (scan_id, limit, user_id),
    )
    for r in rows:
        r["data"] = deserialize(r.get("data"))
    return rows
