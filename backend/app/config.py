from functools import lru_cache
from pathlib import Path

from pydantic_settings import BaseSettings, SettingsConfigDict

ROOT_DIR = Path(__file__).resolve().parent.parent.parent


class Settings(BaseSettings):
    model_config = SettingsConfigDict(env_file=".env", env_file_encoding="utf-8", extra="ignore")

    app_name: str = "AIPentest"
    secret_key: str = "aipentest-dev-secret-key-please-rotate-2026!"
    jwt_algorithm: str = "HS256"
    access_token_expire_minutes: int = 60 * 24 * 7
    database_url: str = str(ROOT_DIR / "backend" / "data" / "aipentest.db")

    # Rust engine
    engine_binary: str = str(ROOT_DIR / "engine" / "target" / "release" / "aipentest-engine")
    engine_timeout: int = 900

    # AI providers (optional)
    openai_api_key: str = ""
    openai_model: str = "gpt-4o-mini"
    openai_base_url: str = "https://api.openai.com/v1"
    gemini_api_key: str = ""
    gemini_model: str = "gemini-2.0-flash"
    hf_api_key: str = ""
    hf_model: str = "meta-llama/Llama-3.2-3B-Instruct"
    ollama_base_url: str = "http://localhost:11434"
    ollama_model: str = "gemma3:4b"
    ai_max_tokens: int = 1024

    default_ai_provider: str = "openai"

    cors_origins: str = "http://localhost:5173,http://localhost:4173"


@lru_cache
def get_settings() -> Settings:
    return Settings()
