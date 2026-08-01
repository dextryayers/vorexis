from pydantic import BaseModel, Field


class RegisterRequest(BaseModel):
    username: str = Field(min_length=3, max_length=32)
    email: str
    password: str = Field(min_length=6)


class LoginRequest(BaseModel):
    username: str
    password: str


class TokenResponse(BaseModel):
    access_token: str
    token_type: str = "bearer"
    username: str


class UserResponse(BaseModel):
    id: str
    username: str
    email: str


class TargetCreate(BaseModel):
    url: str
    label: str | None = None


class TargetResponse(BaseModel):
    id: str
    url: str
    label: str | None = None
    created_at: str


class ScanCreate(BaseModel):
    target: str = Field(min_length=1, max_length=512)
    modules: list[str] = Field(
        default_factory=lambda: ["http", "tls", "fingerprint", "tech", "waf"]
    )
    concurrency: int = Field(default=50, ge=1, le=200)
    timeout: int = Field(default=8, ge=1, le=30)
    options: dict = Field(default_factory=dict)


class ScanResponse(BaseModel):
    id: str
    target: str
    modules: list[str]
    status: str
    progress: float
    started_at: str | None = None
    finished_at: str | None = None
    summary: dict | None = None
    error: str | None = None


class ChatCreate(BaseModel):
    scan_id: str | None = None
    title: str = "New analysis"


class ChatMessageIn(BaseModel):
    chat_id: str
    message: str


class ChatScanAttach(BaseModel):
    scan_id: str


class ChatResponse(BaseModel):
    id: str
    title: str
    scan_id: str | None = None
    created_at: str
    updated_at: str


class ChatMessageOut(BaseModel):
    role: str
    content: str
    created_at: str
