"""
WebSocket transport (client and server).

:class:`WebSocketTransport` dials a ``ws://`` or ``wss://`` endpoint.
:class:`WebSocketServerTransport` wraps a connection yielded by
:class:`WebSocketListener`, which binds a server and accepts inbound clients.
"""

from __future__ import annotations

import asyncio
import logging
from typing import TYPE_CHECKING

import msgpack
from websockets.exceptions import ConnectionClosed, WebSocketException

from saikuro.transport.base import BaseTransport
from saikuro.transport.framing import _MAX_FRAME_SIZE, _check_frame_size

if TYPE_CHECKING:
    from typing import Self

    from websockets.asyncio.client import ClientConnection
    from websockets.asyncio.server import Server, ServerConnection

logger = logging.getLogger(__name__)

_DEFAULT_OPEN_TIMEOUT = 10.0
"""Seconds to wait for the opening handshake."""

_DEFAULT_PING_INTERVAL = 20.0
"""Seconds between keepalive pings; ``None`` disables them."""


class _ListenerClosed:
    """
    Sentinel type queued by :meth:`WebSocketListener.close`
    """


_LISTENER_CLOSED = _ListenerClosed()
"""Queue sentinel meaning the listener is closed and no transport will arrive."""


class _WebSocketConnection(BaseTransport):
    """
    Shared send/recv/shutdown for transports backed by a websockets connection.
    """

    def __init__(self) -> None:
        self._ws: ClientConnection | ServerConnection | None = None
        self._closed = False

    @property
    def is_connected(self) -> bool:
        """Whether the underlying WebSocket connection is live."""
        return not self._closed and self._ws is not None

    def _mark_closed(self) -> None:
        """Record that the peer is gone so the connection is not reused."""
        self._closed = True
        self._ws = None

    async def close(self) -> None:
        """Close the connection. Idempotent, and never raises."""
        if self._closed:
            return
        self._closed = True
        ws = self._ws
        self._ws = None
        if ws is None:
            return
        try:
            await ws.close()
        except Exception:
            logger.debug(
                "%s.close: error during shutdown",
                type(self).__name__,
                exc_info=True,
            )

    async def send(self, obj: dict) -> None:
        if self._closed or self._ws is None:
            raise RuntimeError(f"{type(self).__name__}: not connected")
        data: bytes = msgpack.packb(obj, use_bin_type=True)
        _check_frame_size(data)
        try:
            await self._ws.send(data)
        except Exception as exc:
            raise RuntimeError(f"{type(self).__name__}: send failed: {exc}") from exc

    async def recv(self) -> dict | None:
        if self._closed or self._ws is None:
            return None
        try:
            message = await self._ws.recv()
        except ConnectionClosed:
            logger.debug("%s: connection closed by peer", type(self).__name__)
            self._mark_closed()
            return None
        except (OSError, WebSocketException) as exc:
            logger.warning("%s: recv error: %s", type(self).__name__, exc)
            self._mark_closed()
            return None
        if not isinstance(message, (bytes, bytearray)):
            # Text frame - unexpected for binary MessagePack protocol.
            logger.warning(
                "%s: received unexpected text frame (%d chars), skipping",
                type(self).__name__,
                len(message),
            )
            return None
        try:
            return msgpack.unpackb(bytes(message), raw=False)
        except (ValueError, TypeError, msgpack.exceptions.UnpackException) as exc:
            logger.warning(
                "%s: failed to decode MessagePack frame: %s",
                type(self).__name__,
                exc,
            )
            return None


class WebSocketTransport(_WebSocketConnection):
    """
    Connects to a Saikuro runtime over WebSocket (``ws://`` or ``wss://``).

    Each send delivers one binary frame carrying a raw MessagePack object with
    no additional length-prefix framing.

    Usage::

        transport = WebSocketTransport("ws://localhost:8765/saikuro")
        async with transport:
            await transport.send({"type": "call", ...})
            response = await transport.recv()
    """

    def __init__(
        self,
        uri: str,
        *,
        max_size: int = _MAX_FRAME_SIZE,
        extra_headers: dict[str, str] | None = None,
        open_timeout: float = _DEFAULT_OPEN_TIMEOUT,
        ping_interval: float | None = _DEFAULT_PING_INTERVAL,
        ping_timeout: float | None = _DEFAULT_PING_INTERVAL,
    ) -> None:
        super().__init__()
        self._uri = uri
        self._max_size = max_size
        self._extra_headers = extra_headers
        self._open_timeout = open_timeout
        self._ping_interval = ping_interval
        self._ping_timeout = ping_timeout

    async def connect(self) -> None:
        from websockets.asyncio.client import connect as ws_connect

        kwargs: dict = {
            "max_size": self._max_size,
            "open_timeout": self._open_timeout,
            "ping_interval": self._ping_interval,
            "ping_timeout": self._ping_timeout,
            # Disable per-message deflate - it adds latency and the Saikuro
            # wire protocol already uses a compact binary encoding.
            "compression": None,
        }
        if self._extra_headers is not None:
            kwargs["additional_headers"] = self._extra_headers

        try:
            self._ws = await ws_connect(self._uri, **kwargs)
        except (WebSocketException, OSError) as exc:
            raise RuntimeError(
                f"WebSocketTransport: failed to connect to {self._uri!r}: {exc}"
            ) from exc
        self._closed = False
        logger.debug("WebSocketTransport: connected to %s", self._uri)


class WebSocketServerTransport(_WebSocketConnection):
    """
    A Saikuro transport over a connection accepted by :class:`WebSocketListener`.
    """

    def __init__(self, ws: ServerConnection) -> None:
        super().__init__()
        self._ws = ws

    async def connect(self) -> None:
        """No-op: the connection is live from the moment it is accepted."""
        if self._closed or self._ws is None:
            raise RuntimeError("WebSocketServerTransport: connection is closed")


class WebSocketListener:
    """
    A bound WebSocket server that yields a :class:`WebSocketServerTransport`
    for every accepted connection.

    Usage::

        listener = await WebSocketListener.bind(host="127.0.0.1", port=0)
        try:
            transport = await listener.accept()
            await provider.serve_on(transport)
        finally:
            await listener.close()
    """

    def __init__(
        self,
        server: Server,
        pending: asyncio.Queue[WebSocketServerTransport | _ListenerClosed],
        bound_port: int,
    ) -> None:
        self._server = server
        self._pending = pending
        self._bound_port = bound_port
        self._closed = False
        self._accept_waiters = 0

    @classmethod
    async def bind(cls, host: str = "127.0.0.1", port: int = 0) -> WebSocketListener:
        """Bind a listener. Pass ``port=0`` to let the OS choose a free port."""
        from websockets.asyncio.server import serve as ws_serve

        pending: asyncio.Queue[WebSocketServerTransport | _ListenerClosed] = (
            asyncio.Queue()
        )

        async def handler(ws: ServerConnection) -> None:
            transport = WebSocketServerTransport(ws)
            await pending.put(transport)
            try:
                # Park until the connection goes away so websockets does not
                # close it the moment this handler returns.
                await ws.wait_closed()
            finally:
                transport._mark_closed()

        server = await ws_serve(handler, host, port, compression=None)
        sockets = server.sockets or ()
        bound_port = sockets[0].getsockname()[1] if sockets else port
        return cls(server, pending, bound_port)

    @property
    def port(self) -> int:
        """The port the listener is bound to."""
        return self._bound_port

    async def accept(self) -> WebSocketServerTransport:
        """Wait for the next inbound connection.

        Raises:
            RuntimeError: the listener is closed, so no connection will arrive.
        """
        if self._closed:
            raise RuntimeError("WebSocketListener is closed")
        self._accept_waiters += 1
        try:
            item = await self._pending.get()
        finally:
            self._accept_waiters -= 1
        # Identity, not isinstance: _LISTENER_CLOSED is a singleton sentinel and
        # a closed listener is a runtime state, not a bad argument type.
        if item is _LISTENER_CLOSED:
            raise RuntimeError("WebSocketListener is closed")
        return item

    async def close(self) -> None:
        """Stop listening, close accepted connections, and release handlers."""
        if self._closed:
            return
        self._closed = True
        self._server.close()
        await self._server.wait_closed()
        await self._discard_pending()

    async def _discard_pending(self) -> None:
        """Close unaccepted transports and wake callers parked in :meth:`accept`."""
        while True:
            try:
                item = self._pending.get_nowait()
            except asyncio.QueueEmpty:
                break
            if item is not _LISTENER_CLOSED:
                await item.close()
        for _ in range(self._accept_waiters):
            self._pending.put_nowait(_LISTENER_CLOSED)

    async def __aenter__(self) -> Self:
        return self

    async def __aexit__(self, *_: object) -> None:
        await self.close()
