"""AI service: picks a provider (with a real fallback chain), builds pentest-aware
prompts and optionally includes scan context."""
import json
import logging
from typing import AsyncIterator

from app.config import get_settings
from app.services.ai.base import AIProvider
from app.services.ai.providers import GeminiProvider, HuggingFaceProvider, OllamaProvider, OpenAIProvider

log = logging.getLogger("ai")

DEFAULT_SYSTEM = """You are AIPentest, an expert web application security analyst assistant.
You explain scan findings in clear, structured, actionable terms.
Always be precise: separate confirmed findings from suspicions, and clearly state
severity, impact, and remediation steps. Never run commands yourself; only analyze.

You respond in the same language the user uses."""

PROVIDER_ORDER = ["openai", "gemini", "ollama", "huggingface"]


class AIManager:
    def __init__(self) -> None:
        self._providers: dict[str, AIProvider] = {
            p.name: p for p in (OpenAIProvider(), GeminiProvider(), OllamaProvider(), HuggingFaceProvider())
        }

    def provider_chain(self, requested: str | None = None) -> list[AIProvider]:
        """Ordered candidates: requested → configured default → the rest."""
        settings = get_settings()
        names = []
        if requested:
            names.append(requested)
        if settings.default_ai_provider not in names:
            names.append(settings.default_ai_provider)
        names += [p for p in PROVIDER_ORDER if p not in names]
        chain = [
            self._providers[n]
            for n in names
            if n in self._providers and self._providers[n].available()
        ]
        # Ollama has no key requirement — always keep it as a final candidate.
        if all(p.name != "ollama" for p in chain):
            chain.append(self._providers["ollama"])
        return chain

    async def chat_stream(self, messages: list[dict], system: str | None = None) -> AsyncIterator[str]:
        """Stream from the first working provider; fall through on failure."""
        errors: list[str] = []
        for provider in self.provider_chain():
            try:
                async for chunk in provider.chat_stream(messages, system or DEFAULT_SYSTEM):
                    yield chunk
                return
            except Exception as exc:  # noqa: BLE001
                errors.append(f"{provider.name}: {exc}")
                log.warning("provider %s failed: %s", provider.name, exc)
        raise RuntimeError("All AI providers failed. " + " | ".join(errors))

    async def chat(self, messages: list[dict], system: str | None = None) -> str:
        chunks = []
        async for c in self.chat_stream(messages, system):
            chunks.append(c)
        return "".join(chunks)

    def build_scan_context(self, summary: dict | None, target: str) -> str:
        if not summary or not summary.get("modules"):
            return f"Target: {target}. No scan data available."
        compact = []
        for m in summary["modules"]:
            compact.append({"module": m["module"], "findings": m.get("details", [])[:4]})
        return (
            f"Target: {target}\n"
            f"Scan summary (JSON):\n{json.dumps(compact, indent=2, default=str)[:6000]}"
        )


manager = AIManager()
