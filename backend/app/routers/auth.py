from fastapi import APIRouter, Depends, HTTPException, Request

from app.core.database import execute, fetch_one, new_id, now_iso
from app.core.deps import get_current_user
from app.core.ratelimit import rate_limited
from app.core.security import create_token, hash_password, verify_password
from app.models.schemas import LoginRequest, RegisterRequest, TokenResponse, UserResponse

router = APIRouter(prefix="/api/auth", tags=["auth"])


def _client_key(request: Request) -> str:
    return request.client.host if request.client else "unknown"


@router.post("/register", response_model=TokenResponse)
async def register(body: RegisterRequest, request: Request):
    if rate_limited(f"register:{_client_key(request)}", max_per_minute=10):
        raise HTTPException(429, "too many registration attempts — try again later")
    user_id = new_id()
    try:
        await execute(
            "INSERT INTO users (id, username, email, password_hash, created_at) VALUES (?, ?, ?, ?, ?)",
            (user_id, body.username, body.email, hash_password(body.password), now_iso()),
        )
    except Exception:
        # Race-safe duplicate check: rely on the UNIQUE constraints.
        existing = await fetch_one(
            "SELECT id FROM users WHERE username=? OR email=?", (body.username, body.email)
        )
        if existing:
            raise HTTPException(409, "username or email already registered")
        raise
    return TokenResponse(access_token=create_token(user_id, body.username), username=body.username)


@router.post("/login", response_model=TokenResponse)
async def login(body: LoginRequest, request: Request):
    if rate_limited(f"login:{_client_key(request)}", max_per_minute=20):
        raise HTTPException(429, "too many login attempts — try again later")
    row = await fetch_one("SELECT * FROM users WHERE username=?", (body.username,))
    if not row or not verify_password(body.password, row["password_hash"]):
        raise HTTPException(401, "invalid credentials")
    return TokenResponse(access_token=create_token(row["id"], row["username"]), username=row["username"])


@router.get("/me", response_model=UserResponse)
async def me(user_id: str = Depends(get_current_user)):
    row = await fetch_one("SELECT id, username, email FROM users WHERE id=?", (user_id,))
    if not row:
        raise HTTPException(404, "user not found")
    return UserResponse(**row)
