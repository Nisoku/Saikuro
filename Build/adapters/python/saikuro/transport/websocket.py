"""
WebSocket transport.
"""

from __future__ import annotations

import logging
from typing import Optional

import msgpack
from websockets.exceptions import ConnectionClosed

from saikuro.transport.base import BaseTransport
from saikuro.transport.framing import _MAX_FRAME_SIZE, _check_frame_size


logger = logging.getLogger(__name__)


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
        self._ws: object | None = None
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
        _check_frame_size(data)
        try:
            await self._ws.send(data)  # type: ignore[union-attr]
        except Exception as exc:
            raise RuntimeError(f"WebSocketTransport: send failed: {exc}") from exc

    async def recv(self) -> Optional[dict]:
        if self._closed or self._ws is None:
            return None
        try:
            message = await self._ws.recv()  # type: ignore[union-attr]
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
