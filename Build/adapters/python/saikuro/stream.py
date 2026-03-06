"""
Async iterators for Saikuro streams and channels.
"""

from __future__ import annotations

import asyncio
from typing import Any, AsyncIterator, Optional

from .envelope import ResponseEnvelope
from .error import SaikuroError, StreamClosedError, ChannelClosedError


class SaikuroStream:
    """
    An async iterator that yields items from a server-to-client stream.

    Usage::

        stream = await client.stream("events.subscribe", [])
        async for event in stream:
            process(event)
    """

    def __init__(self, invocation_id: str) -> None:
        self._id = invocation_id
        self._queue: asyncio.Queue[Optional[ResponseEnvelope]] = asyncio.Queue()
        self._closed = False

    @property
    def invocation_id(self) -> str:
        return self._id

    def _deliver(self, response: ResponseEnvelope) -> None:
        """Called by the client receive loop to push items into the stream."""
        self._queue.put_nowait(response)

    def _close(self) -> None:
        """Signal end-of-stream by pushing a sentinel None."""
        if not self._closed:
            self._closed = True
            self._queue.put_nowait(None)

    def __aiter__(self) -> AsyncIterator[Any]:
        return self

    async def __anext__(self) -> Any:
        if self._closed and self._queue.empty():
            raise StopAsyncIteration

        item = await self._queue.get()

        if item is None:
            raise StopAsyncIteration

        if not item.ok:
            if item.error:
                raise SaikuroError.from_error_dict(item.error)
            raise StreamClosedError("stream", "stream ended with error", {})

        if item.is_stream_end:
            raise StopAsyncIteration

        return item.result


class SaikuroChannel:
    """
    A bidirectional async channel.

    Usage::

        channel = await client.channel("chat.open", [])
        await channel.send({"text": "Hello"})
        async for msg in channel:
            print(msg)
    """

    def __init__(self, invocation_id: str, send_fn) -> None:
        self._id = invocation_id
        self._inbound: asyncio.Queue[Optional[ResponseEnvelope]] = asyncio.Queue()
        self._send_fn = send_fn  # coroutine: (value) -> None
        self._closed = False

    @property
    def invocation_id(self) -> str:
        return self._id

    async def send(self, value: Any) -> None:
        """Send a message to the provider side of the channel."""
        if self._closed:
            raise ChannelClosedError("ChannelClosed", "channel is already closed", {})
        await self._send_fn(self._id, value)

    def _deliver(self, response: ResponseEnvelope) -> None:
        self._inbound.put_nowait(response)

    def _close(self) -> None:
        if not self._closed:
            self._closed = True
            self._inbound.put_nowait(None)

    def __aiter__(self) -> AsyncIterator[Any]:
        return self

    async def __anext__(self) -> Any:
        if self._closed and self._inbound.empty():
            raise StopAsyncIteration

        item = await self._inbound.get()

        if item is None:
            raise StopAsyncIteration

        if not item.ok:
            if item.error:
                raise SaikuroError.from_error_dict(item.error)
            raise ChannelClosedError("ChannelClosed", "channel closed with error", {})

        if item.is_stream_end:
            raise StopAsyncIteration

        return item.result
