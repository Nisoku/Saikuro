"""
In-memory transport (for testing).
"""

from __future__ import annotations

import asyncio
from typing import Optional

from saikuro.transport.base import BaseTransport


class InMemoryTransport(BaseTransport):
    """
    In-process transport backed by asyncio Queues.

    Create a connected pair with `InMemoryTransport.pair()`.  The two
    transport objects share two queues and route messages between each other.

    Closing the transport puts a ``None`` sentinel into the receive queue so
    that any coroutine currently blocked on ``recv()`` unblocks immediately
    and gets ``None`` (EOF) rather than hanging forever.
    """

    # Sentinel placed in the recv queue to signal EOF.
    _EOF = object()

    def __init__(
        self,
        send_queue: "asyncio.Queue[object]",
        recv_queue: "asyncio.Queue[object]",
    ) -> None:
        self._send_queue = send_queue
        self._recv_queue = recv_queue
        self._closed = False

    @classmethod
    def pair(cls) -> "tuple[InMemoryTransport, InMemoryTransport]":
        q_a: asyncio.Queue[object] = asyncio.Queue()
        q_b: asyncio.Queue[object] = asyncio.Queue()
        return cls(q_a, q_b), cls(q_b, q_a)

    async def connect(self) -> None:
        pass  # always "connected"

    async def close(self) -> None:
        if not self._closed:
            self._closed = True
            # Unblock any coroutine waiting in recv().
            await self._recv_queue.put(self._EOF)
            # Remove from transport registry if this is a named memory transport.
            from saikuro.transport import make_transport

            make_transport._remove_channel(self)

    async def send(self, obj: dict) -> None:
        if self._closed:
            raise RuntimeError("InMemoryTransport: transport is closed")
        await self._send_queue.put(obj)

    async def recv(self) -> Optional[dict]:
        if self._closed:
            return None
        item = await self._recv_queue.get()
        if item is self._EOF:
            self._closed = True
            return None
        return item  # type: ignore[return-value]
