"""Async SQLite persistence layer (aiosqlite) with a small connection pool.

Local-first design: WAL mode + NORMAL synchronous, a pool of a few
connections for reads, and a single-writer lock so app writes never
collide ("database is locked" is impossible between our own connections).
"""
import asyncio
import json
import sqlite3
import uuid
from datetime import datetime, timezone
from pathlib import Path

import aiosqlite

POOL_SIZE = 4
BUSY_TIMEOUT_MS = 30_000

_pool: list[aiosqlite.Connection] = []
_pool_round: int = 0
_pool_lock: asyncio.Lock | None = None
_write_lock: asyncio.Lock | None = None

SCHEMA = """
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS targets (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    url TEXT NOT NULL,
    label TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE TABLE IF NOT EXISTS scans (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    target_id TEXT,
    target TEXT NOT NULL,
    modules TEXT NOT NULL,
    options TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    progress REAL DEFAULT 0,
    started_at TEXT,
    finished_at TEXT,
    summary TEXT,
    error TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);
CREATE TABLE IF NOT EXISTS scan_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id TEXT NOT NULL,
    module TEXT NOT NULL,
    event_type TEXT NOT NULL,
    data TEXT,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS chats (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    scan_id TEXT,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS chat_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scan_events_scan ON scan_events(scan_id);
CREATE INDEX IF NOT EXISTS idx_scans_user ON scans(user_id);
CREATE INDEX IF NOT EXISTS idx_scans_started ON scans(started_at);
CREATE INDEX IF NOT EXISTS idx_targets_user ON targets(user_id);
CREATE INDEX IF NOT EXISTS idx_chats_user ON chats(user_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_chat ON chat_messages(chat_id);
"""


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def new_id() -> str:
    return uuid.uuid4().hex


async def _configure(conn: aiosqlite.Connection) -> None:
    await conn.execute("PRAGMA journal_mode=WAL")
    await conn.execute("PRAGMA synchronous=NORMAL")
    await conn.execute(f"PRAGMA busy_timeout={BUSY_TIMEOUT_MS}")
    await conn.execute("PRAGMA foreign_keys=ON")
    await conn.execute("PRAGMA cache_size=-8192")


async def init_db(db_path: str) -> None:
    global _pool_round, _pool_lock, _write_lock
    Path(db_path).parent.mkdir(parents=True, exist_ok=True)
    _pool.clear()
    for _ in range(POOL_SIZE):
        # True autocommit: every statement is its own transaction, so no
        # connection can ever hold an implicit transaction open.
        conn = await aiosqlite.connect(db_path, isolation_level=None)
        conn.row_factory = aiosqlite.Row
        await _configure(conn)
        _pool.append(conn)
    await _pool[0].executescript(SCHEMA)
    await _pool[0].commit()
    await _pool[0].execute("PRAGMA wal_checkpoint(TRUNCATE)")
    _pool_lock = asyncio.Lock()
    _write_lock = asyncio.Lock()
    _pool_round = 0


def _next_conn() -> aiosqlite.Connection:
    """Round-robin connection selection (caller holds the lock for writes)."""
    if not _pool:
        raise RuntimeError("database not initialized — call init_db first")
    global _pool_round
    conn = _pool[_pool_round % len(_pool)]
    _pool_round += 1
    return conn


async def get_db() -> aiosqlite.Connection:
    if not _pool:
        raise RuntimeError("database not initialized — call init_db first")
    return _pool[0]


async def fetch_one(sql: str, params: tuple = ()) -> dict | None:
    conn = _next_conn()
    try:
        cur = await conn.execute(sql, params)
        row = await cur.fetchone()
        await cur.close()
        return dict(row) if row else None
    finally:
        await conn.rollback()


async def fetch_all(sql: str, params: tuple = ()) -> list[dict]:
    conn = _next_conn()
    try:
        cur = await conn.execute(sql, params)
        rows = await cur.fetchall()
        await cur.close()
        return [dict(r) for r in rows]
    finally:
        await conn.rollback()


def _is_locked(exc: BaseException) -> bool:
    return isinstance(exc, sqlite3.OperationalError) and "locked" in str(exc)


async def execute(sql: str, params: tuple = ()) -> None:
    """Serialized write: one writer at a time, with retry on lock."""
    assert _write_lock is not None
    async with _write_lock:
        for attempt in range(4):
            conn = _next_conn()
            try:
                await conn.execute(sql, params)
                await conn.commit()
                return
            except BaseException as exc:  # noqa: BLE001
                if not _is_locked(exc) or attempt == 3:
                    raise
                await asyncio.sleep(0.05 * (attempt + 1))


async def execute_many(sql: str, params: list[tuple]) -> None:
    """Serialized bulk write with retry on lock."""
    assert _write_lock is not None
    async with _write_lock:
        for attempt in range(4):
            conn = _next_conn()
            try:
                await conn.executemany(sql, params)
                await conn.commit()
                return
            except BaseException as exc:  # noqa: BLE001
                if not _is_locked(exc) or attempt == 3:
                    raise
                await asyncio.sleep(0.05 * (attempt + 1))


def serialize(data) -> str:
    return json.dumps(data, default=str)


def deserialize(raw: str | None) -> dict | list | None:
    if not raw:
        return None
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return None
