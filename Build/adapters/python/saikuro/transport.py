"""
Transport abstraction for the Python Saikuro adapter.

Implementations:
  - UnixSocketTransport  (same-machine, Unix only)
  - TcpTransport         (cross-machine)
  - WebSocketTransport   (ws:// or wss://)
  - InMemoryTransport    (in-process testing)
"""

from __future__ import annotations

import asyncio
import logging
import struct
import msgpack
from abc import ABC, abstractmethod
from typing import Optional


logger = logging.getLogger(__name__)


#  Frame codec

_LENGTH_HEADER = struct.Struct(">I")  # big-endian uint32
_MAX_FRAME_SIZE = 16 * 1024 * 1024  # 16 MiB - matches the Rust framing codec


async def _send_frame(writer: asyncio.StreamWriter, data: bytes) -> None:
    if len(data) > _MAX_FRAME_SIZE:
        raise ValueError(
            f"frame {len(data)} bytes exceeds maximum {_MAX_FRAME_SIZE} bytes"
        )
    header = _LENGTH_HEADER.pack(len(data))
    writer.write(header + data)
    await writer.drain()


async def _recv_frame(reader: asyncio.StreamReader) -> Optional[bytes]:
    """Read one length-prefixed frame.

    Returns ``None`` on clean EOF (peer closed the connection).
    Raises ``ValueError`` if the declared frame length exceeds the limit.
    Propagates ``asyncio.IncompleteReadError`` if the peer closed mid-frame
    (callers should treat this as a transport error, not a clean close).
    """
    try:
        header = await reader.readexactly(4)
    except asyncio.IncompleteReadError as exc:
        if not exc.partial:
            # Clean EOF - the peer closed the connection gracefully.
            return None
        raise
    (length,) = _LENGTH_HEADER.unpack(header)
    if length > _MAX_FRAME_SIZE:
        raise ValueError(
            f"incoming frame claims {length} bytes - exceeds maximum {_MAX_FRAME_SIZE}"
        )
    if length == 0:
        return b""
    return await reader.readexactly(length)


#  Base class


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


#  Stream-based transport (shared by UnixSocket and TCP)


class _StreamTransport(BaseTransport):
    """Shared plumbing for transports backed by ``asyncio.StreamReader`` /
    ``asyncio.StreamWriter`` (Unix domain sockets and TCP).

    Subclasses only need to implement :meth:`connect`; everything else
    (framed send/recv, close with error handling) is provided here.
    """

    def __init__(self) -> None:
        self._reader: Optional[asyncio.StreamReader] = None
        self._writer: Optional[asyncio.StreamWriter] = None

    async def close(self) -> None:
        if self._writer is not None:
            writer = self._writer
            self._writer = None
            self._reader = None
            try:
                writer.close()
                await writer.wait_closed()
            except Exception:
                logger.debug(
                    "%s.close: error during shutdown", type(self).__name__, exc_info=True
                )

    async def send(self, obj: dict) -> None:
        if self._writer is None:
            raise RuntimeError(f"{type(self).__name__}: not connected")
        data = msgpack.packb(obj, use_bin_type=True)
        await _send_frame(self._writer, data)

    async def recv(self) -> Optional[dict]:
        if self._reader is None:
            raise RuntimeError(f"{type(self).__name__}: not connected")
        try:
            data = await _recv_frame(self._reader)
        except asyncio.IncompleteReadError as exc:
            logger.warning("%s: connection lost mid-frame: %s", type(self).__name__, exc)
            return None
        if data is None:
            return None
        return msgpack.unpackb(data, raw=False)


#  Unix socket transport


class UnixSocketTransport(_StreamTransport):
    """Connects to a Saikuro runtime over a Unix domain socket."""

    def __init__(self, path: str) -> None:
        super().__init__()
        self._path = path

    async def connect(self) -> None:
        self._reader, self._writer = await asyncio.open_unix_connection(self._path)


#  TCP transport


class TcpTransport(_StreamTransport):
    """Connects to a Saikuro runtime over TCP."""

    def __init__(self, host: str, port: int) -> None:
        super().__init__()
        self._host = host
        self._port = port

    async def connect(self) -> None:
        self._reader, self._writer = await asyncio.open_connection(
            self._host, self._port
        )


#  WebSocket transport

import websockets


class WebSocketTransport(BaseTransport):
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
        extra_headers: "dict[str, str] | None" = None,
        open_timeout: float = 10.0,
        ping_interval: "float | None" = 20.0,
        ping_timeout: "float | None" = 20.0,
    ) -> None:
        self._uri = uri
        self._max_size = max_size
        self._extra_headers = extra_headers
        self._open_timeout = open_timeout
        self._ping_interval = ping_interval
        self._ping_timeout = ping_timeout
        self._ws: "object | None" = None  # websockets.asyncio.client.ClientConnection
        self._closed = False

    async def connect(self) -> None:
        from websockets.asyncio.client import connect as ws_connect
        from websockets.exceptions import WebSocketException

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

    async def close(self) -> None:
        if self._closed or self._ws is None:
            return
        self._closed = True
        ws = self._ws
        self._ws = None
        try:
            await ws.close()  # type: ignore[union-attr]
        except Exception:
            logger.debug(
                "WebSocketTransport.close: error during shutdown", exc_info=True
            )

    async def send(self, obj: dict) -> None:
        if self._closed or self._ws is None:
            raise RuntimeError("WebSocketTransport: not connected")
        data: bytes = msgpack.packb(obj, use_bin_type=True)
        if len(data) > _MAX_FRAME_SIZE:
            raise ValueError(
                f"frame {len(data)} bytes exceeds maximum {_MAX_FRAME_SIZE} bytes"
            )
        try:
            await self._ws.send(data)  # type: ignore[union-attr]
        except Exception as exc:
            raise RuntimeError(f"WebSocketTransport: send failed: {exc}") from exc

    async def recv(self) -> Optional[dict]:
        if self._closed or self._ws is None:
            return None
        try:
            from websockets.exceptions import ConnectionClosed

            message = await self._ws.recv(decode=False)  # type: ignore[union-attr]
        except ConnectionClosed:
            logger.debug("WebSocketTransport: connection closed by peer")
            self._closed = True
            return None
        except Exception as exc:
            logger.warning("WebSocketTransport: recv error: %s", exc)
            self._closed = True
            return None
        if not isinstance(message, (bytes, bytearray)):
            # Text frame - unexpected for binary MessagePack protocol.
            logger.warning(
                "WebSocketTransport: received unexpected text frame (%d chars), skipping",
                len(message),
            )
            return None
        try:
            return msgpack.unpackb(bytes(message), raw=False)
        except Exception as exc:
            logger.warning(
                "WebSocketTransport: failed to decode MessagePack frame: %s", exc
            )
            return None


#  In-memory transport (for testing)


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


#  Factory function
_memory_channels: dict[str, "InMemoryTransport"] = {}


def make_transport(address: str) -> BaseTransport:
    """
    Construct the best transport for `address`.

    Address format:
      - ``unix:///path/to/socket``  - Unix domain socket
      - ``tcp://host:port``         - TCP
      - ``ws://host:port/path``     - WebSocket (plain)
      - ``wss://host:port/path``    - WebSocket (TLS)
      - ``memory://``               - In-memory (testing only; creates default channel)
      - ``memory://channel-name``   - In-memory with named channel

    In-memory channels work as pairs:
      - First call to ``memory://foo`` creates a pair and returns side A
      - Second call to ``memory://foo`` returns side B, connected to A
      - For tests, prefer ``InMemoryTransport.pair()`` for direct control
    """
    global _memory_channels

    if address == "memory://" or address.startswith("memory://"):
        name = address[len("memory://") :] if len(address) > len("memory://") else "default"
        if name in _memory_channels:
            return _memory_channels.pop(name)
        else:
            a, b = InMemoryTransport.pair()
            _memory_channels[name] = b
            return a

    if address.startswith("unix://"):
        path = address[len("unix://") :]
        return UnixSocketTransport(path)
    if address.startswith("tcp://"):
        rest = address[len("tcp://") :]
        host, _, port_str = rest.rpartition(":")
        return TcpTransport(host, int(port_str))
    if address.startswith("ws://") or address.startswith("wss://"):
        return WebSocketTransport(address)
    raise ValueError(
        f"unsupported transport address: {address!r}\n"
        "Supported schemes: unix://, tcp://, ws://, wss://, memory://"
    )
