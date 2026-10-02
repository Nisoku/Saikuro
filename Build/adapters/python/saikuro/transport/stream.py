"""
Stream-based transport (shared by UnixSocket and TCP).
"""

from __future__ import annotations

import asyncio
import logging
from typing import Self

import msgpack

from saikuro.transport.base import BaseTransport
from saikuro.transport.framing import _recv_frame, _send_frame

logger = logging.getLogger(__name__)


class _StreamTransport(BaseTransport):
    """Shared plumbing for transports backed by ``asyncio.StreamReader`` /
    ``asyncio.StreamWriter`` (Unix domain sockets and TCP).

    Subclasses only need to implement :meth:`_dial`; everything else
    (idempotent connect, framed send/recv, close with error handling) is
    provided here.
    """

    def __init__(self) -> None:
        self._reader: asyncio.StreamReader | None = None
        self._writer: asyncio.StreamWriter | None = None
        self._connected = False
        self._adopted = False

    @classmethod
    def from_stream(
        cls,
        reader: asyncio.StreamReader,
        writer: asyncio.StreamWriter,
    ) -> Self:
        """Adopt an already-connected stream pair, for example from a listener."""
        transport = cls.__new__(cls)
        _StreamTransport.__init__(transport)
        transport._reader = reader
        transport._writer = writer
        transport._connected = True
        transport._adopted = True
        return transport

    @property
    def is_connected(self) -> bool:
        """Whether the transport holds a live stream pair."""
        return self._connected

    async def connect(self) -> None:
        """Bring the connection up, or leave a live one untouched.

        ``SaikuroClient.open_on`` connects unconditionally, so an already-live
        transport must not dial a second socket and orphan the one it serves.

        Raises `RuntimeError` when the stream was adopted via
        :meth:`from_stream` and has since gone away: adoption carries no dial
        target, so there is nothing to reconnect to.
        """
        if self._connected:
            return
        if self._adopted:
            raise RuntimeError(
                f"{type(self).__name__}: cannot reconnect a stream adopted via "
                "from_stream, it has no dial target"
            )
        await self._dial()

    async def _dial(self) -> None:
        raise NotImplementedError

    async def _drop_stream(self) -> None:
        """Release the stream pair and mark the connection dead."""
        writer = self._writer
        self._reader = None
        self._writer = None
        self._connected = False
        if writer is not None:
            try:
                writer.close()
                await writer.wait_closed()
            except Exception:
                logger.debug(
                    "%s: error during shutdown",
                    type(self).__name__,
                    exc_info=True,
                )

    async def close(self) -> None:
        await self._drop_stream()

    async def send(self, obj: dict) -> None:
        if self._writer is None:
            raise RuntimeError(f"{type(self).__name__}: not connected")
        data = msgpack.packb(obj, use_bin_type=True)
        await _send_frame(self._writer, data)

    async def recv(self) -> dict | None:
        if self._reader is None:
            raise RuntimeError(f"{type(self).__name__}: not connected")
        try:
            data = await _recv_frame(self._reader)
        except asyncio.IncompleteReadError as exc:
            logger.warning(
                "%s: connection lost mid-frame: %s", type(self).__name__, exc
            )
            await self._drop_stream()
            return None
        if data is None:
            # Peer hung up: the stream is dead, so stop reporting it connected
            # and let a dial transport dial again.
            await self._drop_stream()
            return None
        return msgpack.unpackb(data, raw=False)
