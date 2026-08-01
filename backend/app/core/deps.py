from fastapi import Header, HTTPException

from app.core.security import decode_token


async def get_current_user(authorization: str | None = Header(default=None)) -> str:
    if not authorization or not authorization.startswith("Bearer "):
        raise HTTPException(401, "missing bearer token")
    payload = decode_token(authorization[7:])
    if not payload:
        raise HTTPException(401, "invalid or expired token")
    return payload["sub"]
