"""
Transport abstraction for the Python Saikuro adapter.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Optional


class BaseTransport(ABC):
    """Abstract base for all Saikuro transports."""

    @abstractmethod
    async def connect(self) -> None:
        """Establish the connection."""

    @abstractmethod
    async def close(self) -> None:
        """Close the connection gracefully."""

    @abstractmethod
    async def send(self, obj: dict) -> None:
        """Serialise `obj` to MessagePack and send it as a framed message."""

    @abstractmethod
    async def recv(self) -> Optional[dict]:
        """Receive and deserialise the next framed MessagePack message.

        Returns `None` when the connection has been closed by the peer.
        """

    async def __aenter__(self) -> "BaseTransport":
        await self.connect()
        return self

    async def __aexit__(self, *_: object) -> None:
        await self.close()
