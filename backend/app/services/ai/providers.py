"""Concrete AI providers: OpenAI-compatible, Gemini, Ollama, HuggingFace.

All providers share one pooled httpx client (connection reuse) and a bounded
max_tokens to keep responses predictable.
"""
import json
import logging
from typing import AsyncIterator

import httpx

from app.config import get_settings
from app.services.ai.base import AIProvider

log = logging.getLogger("ai")

_client = httpx.AsyncClient(
    timeout=httpx.Timeout(120.0, connect=10.0),
    limits=httpx.Limits(max_connections=20, max_keepalive_connections=10),
)


class OpenAIProvider(AIProvider):
    """Any OpenAI-compatible API (OpenAI, DeepSeek, OpenRouter, LM Studio, vLLM...)."""

    name = "openai"

    def __init__(self) -> None:
        s = get_settings()
        self.api_key = s.openai_api_key
        self.model = s.openai_model
        self.base_url = s.openai_base_url.rstrip("/")
        self.max_tokens = s.ai_max_tokens

    def available(self) -> bool:
        return bool(self.api_key)

    async def chat_stream(
        self, messages: list[dict], system: str | None = None
    ) -> AsyncIterator[str]:
        full = [{"role": "system", "content": system}] if system else []
        full += messages
        try:
            async with _client.stream(
                "POST",
                f"{self.base_url}/chat/completions",
                headers={"Authorization": f"Bearer {self.api_key}"},
                json={
                    "model": self.model,
                    "messages": full,
                    "stream": True,
                    "temperature": 0.3,
                    "max_tokens": self.max_tokens,
                },
            ) as resp:
                if resp.status_code >= 400:
                    body = (await resp.aread()).decode()[:300]
                    raise RuntimeError(f"OpenAI-compatible API error {resp.status_code}: {body}")
                async for line in resp.aiter_lines():
                    if not line.startswith("data:"):
                        continue
                    payload = line[5:].strip()
                    if payload == "[DONE]":
                        break
                    try:
                        delta = json.loads(payload)["choices"][0]["delta"].get("content", "")
                    except (json.JSONDecodeError, KeyError, IndexError):
                        continue
                    if delta:
                        yield delta
        except httpx.ConnectError as exc:
            raise RuntimeError(f"cannot reach {self.base_url}") from exc


class GeminiProvider(AIProvider):
    name = "gemini"

    def __init__(self) -> None:
        s = get_settings()
        self.api_key = s.gemini_api_key
        self.model = s.gemini_model
        self.max_tokens = s.ai_max_tokens

    def available(self) -> bool:
        return bool(self.api_key)

    async def chat_stream(
        self, messages: list[dict], system: str | None = None
    ) -> AsyncIterator[str]:
        contents = []
        if system:
            contents.append({"role": "user", "parts": [{"text": system}]})
            contents.append({"role": "model", "parts": [{"text": "Understood."}]})
        for m in messages:
            role = "model" if m["role"] == "assistant" else "user"
            contents.append({"role": role, "parts": [{"text": m["content"]}]})

        url = (
            f"https://generativelanguage.googleapis.com/v1beta/models/"
            f"{self.model}:streamGenerateContent?alt=sse&key={self.api_key}"
        )
        try:
            async with _client.stream(
                "POST",
                url,
                json={"contents": contents, "generationConfig": {"maxOutputTokens": self.max_tokens}},
            ) as resp:
                if resp.status_code >= 400:
                    body = (await resp.aread()).decode()[:300]
                    raise RuntimeError(f"Gemini error {resp.status_code}: {body}")
                async for line in resp.aiter_lines():
                    line = line.strip()
                    if not line.startswith("data:"):
                        continue
                    try:
                        data = json.loads(line[5:].strip())
                        text = data["candidates"][0]["content"]["parts"][0]["text"]
                    except (json.JSONDecodeError, KeyError, IndexError):
                        continue
                    if text:
                        yield text
        except httpx.ConnectError as exc:
            raise RuntimeError("cannot reach Google Gemini API") from exc


class OllamaProvider(AIProvider):
    name = "ollama"

    def __init__(self) -> None:
        s = get_settings()
        self.base_url = s.ollama_base_url.rstrip("/")
        self.model = s.ollama_model
        self.max_tokens = s.ai_max_tokens

    def available(self) -> bool:
        return True

    async def chat_stream(
        self, messages: list[dict], system: str | None = None
    ) -> AsyncIterator[str]:
        full = [{"role": "system", "content": system}] if system else []
        full += messages
        try:
            async with _client.stream(
                "POST",
                f"{self.base_url}/api/chat",
                json={
                    "model": self.model,
                    "messages": full,
                    "stream": True,
                    "options": {"num_predict": self.max_tokens},
                },
            ) as resp:
                if resp.status_code >= 400:
                    body = (await resp.aread()).decode()[:300]
                    raise RuntimeError(f"Ollama error {resp.status_code}: {body}")
                async for line in resp.aiter_lines():
                    if not line:
                        continue
                    try:
                        chunk = json.loads(line).get("message", {}).get("content", "")
                    except json.JSONDecodeError:
                        continue
                    if chunk:
                        yield chunk
        except httpx.ConnectError:
            raise RuntimeError(
                f"Ollama not reachable at {self.base_url} — start it with `ollama serve`"
            ) from None


class HuggingFaceProvider(AIProvider):
    name = "huggingface"

    def __init__(self) -> None:
        s = get_settings()
        self.api_key = s.hf_api_key
        self.model = s.hf_model
        self.max_tokens = s.ai_max_tokens

    def available(self) -> bool:
        return bool(self.api_key)

    async def chat_stream(
        self, messages: list[dict], system: str | None = None
    ) -> AsyncIterator[str]:
        prompt = "\n".join(f"{m['role']}: {m['content']}" for m in messages)
        if system:
            prompt = f"System: {system}\n{prompt}\nassistant:"
        try:
            async with _client.stream(
                "POST",
                f"https://api-inference.huggingface.co/models/{self.model}",
                headers={"Authorization": f"Bearer {self.api_key}"},
                json={"inputs": prompt, "parameters": {"max_new_tokens": self.max_tokens}},
            ) as resp:
                if resp.status_code >= 400:
                    body = (await resp.aread()).decode()[:300]
                    raise RuntimeError(f"HuggingFace error {resp.status_code}: {body}")
                text = await resp.aread()
                data = json.loads(text)
                if isinstance(data, list) and data:
                    yield data[0].get("generated_text", "")[len(prompt):]
                elif isinstance(data, dict):
                    yield data.get("generated_text", "")
        except httpx.ConnectError as exc:
            raise RuntimeError("cannot reach HuggingFace API") from exc
