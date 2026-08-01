"""AI provider abstraction. Each provider streams text chunks."""
import logging
from abc import ABC, abstractmethod
from typing import AsyncIterator

log = logging.getLogger("ai")


class AIProvider(ABC):
    name: str = "base"

    @abstractmethod
    def available(self) -> bool:
        """True when this provider is configured and usable."""

    @abstractmethod
    async def chat_stream(self, messages: list[dict], system: str | None = None) -> AsyncIterator[str]:
        """Stream assistant text chunks for the given chat messages."""

    async def chat(self, messages: list[dict], system: str | None = None) -> str:
        chunks = []
        async for c in self.chat_stream(messages, system):
            chunks.append(c)
        return "".join(chunks)
