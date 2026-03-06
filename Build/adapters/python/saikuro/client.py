"""
Async client for Saikuro.
"""

from __future__ import annotations

import asyncio
import datetime
import logging
from typing import Any, Dict, List, Optional, Sequence, Tuple, Union

from .envelope import (
    Envelope,
    InvocationType,
    LogLevel,
    LogRecord,
    ResourceHandle,
    ResponseEnvelope,
)
from .error import SaikuroError, TransportError
from .stream import SaikuroChannel, SaikuroStream
from .transport import BaseTransport, make_transport

logger = logging.getLogger(__name__)


class SaikuroClient:
    """
    An async Saikuro client.

    Manages one transport connection to a Saikuro runtime and multiplexes all
    call, stream, and channel invocations over it.

    Use as an async context manager::

        async with SaikuroClient.connect("unix:///tmp/saikuro.sock") as client:
            result = await client.call("math.add", [1, 2])
    """

    def __init__(self, transport: BaseTransport) -> None:
        self._transport = transport
        # pending calls: invocation_id -> Future[ResponseEnvelope]
        self._pending_calls: Dict[str, asyncio.Future[ResponseEnvelope]] = {}
        # open streams: invocation_id -> SaikuroStream
        self._open_streams: Dict[str, SaikuroStream] = {}
        # open channels: invocation_id -> SaikuroChannel
        self._open_channels: Dict[str, SaikuroChannel] = {}
        self._receive_task: Optional[asyncio.Task] = None
        self._connected = False

    #  Construction

    @classmethod
    async def connect(cls, address: str) -> "SaikuroClient":
        """Connect to a Saikuro runtime at `address` and return a ready client.

        Prefer using this as an async context manager.
        """
        transport = make_transport(address)
        client = cls(transport)
        await client._connect()
        return client

    @classmethod
    def from_transport(cls, transport: BaseTransport) -> "SaikuroClient":
        """Construct a client from an already-instantiated transport."""
        return cls(transport)

    #  Lifecycle

    async def _connect(self) -> None:
        await self._transport.connect()
        self._connected = True
        self._receive_task = asyncio.ensure_future(self._receive_loop())
        logger.debug("saikuro client connected")

    async def close(self) -> None:
        """Close the client and its transport."""
        self._connected = False
        if self._receive_task is not None:
            self._receive_task.cancel()
            try:
                await self._receive_task
            except asyncio.CancelledError:
                pass
        await self._transport.close()

        # Fail any pending calls with a transport error.
        for inv_id, fut in self._pending_calls.items():
            if not fut.done():
                fut.set_exception(
                    TransportError(
                        "ConnectionLost", "client was closed while call was in flight"
                    )
                )
        self._pending_calls.clear()

        # Close open streams and channels.
        for stream in self._open_streams.values():
            stream._close()
        for channel in self._open_channels.values():
            channel._close()

        logger.debug("saikuro client closed")

    async def __aenter__(self) -> "SaikuroClient":
        await self._connect()
        return self

    async def __aexit__(self, *_: object) -> None:
        await self.close()

    #  Invocation API

    async def call(
        self,
        target: str,
        args: List[Any],
        capability: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Any:
        """Perform a request/response call and return the result.

        Raises:
            SaikuroError (or a specific subclass) on failure.
            asyncio.TimeoutError if `timeout` seconds elapse without a response.
        """
        envelope = Envelope.make_call(target, args, capability)
        loop = asyncio.get_running_loop()
        future: asyncio.Future[ResponseEnvelope] = loop.create_future()
        self._pending_calls[envelope.id] = future

        await self._transport.send(envelope.to_msgpack_dict())

        try:
            if timeout is not None:
                response = await asyncio.wait_for(future, timeout=timeout)
            else:
                response = await future
        finally:
            self._pending_calls.pop(envelope.id, None)

        if not response.ok:
            raise SaikuroError.from_error_dict(
                response.error or {"code": "Internal", "message": "no error details"}
            )

        return response.result

    async def cast(
        self,
        target: str,
        args: List[Any],
        capability: Optional[str] = None,
    ) -> None:
        """Fire-and-forget invocation. No response is expected."""
        envelope = Envelope.make_cast(target, args, capability)
        await self._transport.send(envelope.to_msgpack_dict())

    async def batch(
        self,
        calls: Sequence[
            Union[Tuple[str, List[Any]], Tuple[str, List[Any], Optional[str]]]
        ],
        timeout: Optional[float] = None,
    ) -> List[Any]:
        """Send multiple calls in a single batch envelope.

        Each element of `calls` may be a ``(target, args)`` 2-tuple or a
        ``(target, args, capability)`` 3-tuple.  Results are returned in the
        same order as the input.

        A failed batch item is represented as ``None`` in the result list.

        Raises:
            SaikuroError if the batch envelope itself is rejected.
            asyncio.TimeoutError if `timeout` seconds elapse before the response.

        Example::

            results = await client.batch([
                ("math.add", [1, 2]),
                ("math.multiply", [3, 4]),
            ])
            # results == [3, 12]
        """
        items: List[Envelope] = []
        for entry in calls:
            target = entry[0]
            args_list: List[Any] = entry[1]
            cap: Optional[str] = entry[2] if len(entry) > 2 else None  # type: ignore[misc]
            items.append(Envelope.make_call(target, args_list, cap))

        batch_envelope = Envelope.make_batch(items)

        loop = asyncio.get_running_loop()
        future: asyncio.Future[ResponseEnvelope] = loop.create_future()
        self._pending_calls[batch_envelope.id] = future

        await self._transport.send(batch_envelope.to_msgpack_dict())

        try:
            if timeout is not None:
                response = await asyncio.wait_for(future, timeout=timeout)
            else:
                response = await future
        finally:
            self._pending_calls.pop(batch_envelope.id, None)

        if not response.ok:
            raise SaikuroError.from_error_dict(
                response.error or {"code": "Internal", "message": "batch call failed"}
            )

        # The result is an ordered array of per-item results (None for failures).
        raw = response.result
        if isinstance(raw, list):
            return raw
        # Graceful fallback: runtime returned something unexpected.
        return [raw]

    async def resource(
        self,
        target: str,
        args: List[Any],
        capability: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> ResourceHandle:
        """Invoke a provider function that manages an external resource and
        return the resulting class:`~saikuro.envelope.ResourceHandle` .

        Sends a ``resource``-type envelope, waits for a single response (same
        semantics as :meth:`call`), and decodes the ``result`` field into a
        :class:`~saikuro.envelope.ResourceHandle`.

        Raises:
            SaikuroError (or a specific subclass) if the invocation fails.
            ValueError if the provider returns a result that is not a valid
            :class:`~saikuro.envelope.ResourceHandle` map.
            asyncio.TimeoutError if ``timeout`` seconds elapse without a response.

        Example::

            handle = await client.resource("files.open", ["/var/data/report.csv"])
            print(handle.id, handle.mime_type, handle.size, handle.uri)
        """
        envelope = Envelope.make_resource(target, args, capability)
        loop = asyncio.get_running_loop()
        future: asyncio.Future[ResponseEnvelope] = loop.create_future()
        self._pending_calls[envelope.id] = future

        await self._transport.send(envelope.to_msgpack_dict())

        try:
            if timeout is not None:
                response = await asyncio.wait_for(future, timeout=timeout)
            else:
                response = await future
        finally:
            self._pending_calls.pop(envelope.id, None)

        if not response.ok:
            raise SaikuroError.from_error_dict(
                response.error
                or {"code": "Internal", "message": "resource call failed"}
            )

        if not isinstance(response.result, dict):
            raise ValueError(
                f"resource invocation for {target!r} returned a non-dict result: "
                f"{response.result!r}"
            )

        return ResourceHandle.from_dict(response.result)

    async def stream(
        self,
        target: str,
        args: List[Any],
        capability: Optional[str] = None,
    ) -> SaikuroStream:
        """Open a server-to-client stream and return an async iterator.

        The iterator yields values as they arrive from the provider.
        """
        envelope = Envelope.make_stream_open(target, args)
        if capability:
            envelope.capability = capability

        stream_obj = SaikuroStream(envelope.id)
        self._open_streams[envelope.id] = stream_obj
        await self._transport.send(envelope.to_msgpack_dict())
        return stream_obj

    async def channel(
        self,
        target: str,
        args: List[Any],
        capability: Optional[str] = None,
    ) -> SaikuroChannel:
        """Open a bidirectional channel and return a SaikuroChannel."""
        envelope = Envelope.make_channel_open(target, args)
        if capability:
            envelope.capability = capability

        channel_obj = SaikuroChannel(envelope.id, self._channel_send)
        self._open_channels[envelope.id] = channel_obj
        await self._transport.send(envelope.to_msgpack_dict())
        return channel_obj

    async def log(
        self,
        level: LogLevel,
        name: str,
        msg: str,
        fields: Optional[Dict[str, Any]] = None,
    ) -> None:
        """Forward a structured log record to the runtime log sink.

        Fire-and-forget; no response is expected.
        """
        ts = datetime.datetime.now(tz=datetime.timezone.utc).isoformat()
        record = LogRecord(ts=ts, level=level, name=name, msg=msg, fields=fields or {})
        envelope = Envelope(
            version=1,
            invocation_type=InvocationType.LOG,
            id=f"log-{ts}",
            target="$log",
            args=[record.to_dict()],
        )
        await self._transport.send(envelope.to_msgpack_dict())

    async def _channel_send(self, channel_id: str, value: Any) -> None:
        """Send an outbound message on an open channel."""
        envelope = Envelope(
            version=1,
            invocation_type=InvocationType.CHANNEL,
            id=channel_id,
            target="",  # routing is by ID for follow-up channel messages
            args=[value],
        )
        await self._transport.send(envelope.to_msgpack_dict())

    #  Receive loop

    async def _receive_loop(self) -> None:
        """Dispatch every inbound message to the right waiter."""
        while self._connected:
            try:
                raw = await self._transport.recv()
            except Exception as exc:
                if self._connected:
                    logger.error(
                        "saikuro client: transport receive error, connection will be torn down: %s",
                        exc,
                        exc_info=True,
                    )
                break

            if raw is None:
                logger.debug("saikuro client: transport closed by peer")
                break

            try:
                response = ResponseEnvelope.from_msgpack_dict(raw)
            except Exception as exc:
                logger.error(
                    "saikuro client: malformed response envelope, discarding: %s raw=%r",
                    exc,
                    raw,
                    exc_info=True,
                )
                continue

            self._dispatch_response(response)

        # Connection is gone - fail everything still waiting.
        for fut in self._pending_calls.values():
            if not fut.done():
                fut.set_exception(
                    TransportError("ConnectionLost", "transport closed unexpectedly")
                )
        for stream in self._open_streams.values():
            stream._close()
        for channel in self._open_channels.values():
            channel._close()

    def _dispatch_response(self, response: ResponseEnvelope) -> None:
        inv_id = response.id

        # Is it a response to a pending call?
        if inv_id in self._pending_calls:
            fut = self._pending_calls.pop(inv_id)
            if not fut.done():
                fut.set_result(response)
            return

        # Is it a stream item?
        if inv_id in self._open_streams:
            stream = self._open_streams[inv_id]
            stream._deliver(response)
            if response.is_stream_end:
                del self._open_streams[inv_id]
            return

        # Is it a channel message?
        if inv_id in self._open_channels:
            channel = self._open_channels[inv_id]
            channel._deliver(response)
            if response.is_stream_end:
                del self._open_channels[inv_id]
            return

        logger.warning(
            "saikuro client: received response for unknown invocation id %r - "
            "may be a late response after timeout",
            inv_id,
        )
