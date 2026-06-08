"""
Stream-based transport (shared by UnixSocket and TCP).
"""

from __future__ import annotations

import asyncio
import logging
from typing import Optional

import msgpack

from saikuro.transport.base import BaseTransport
from saikuro.transport.framing import _recv_frame, _send_frame


logger = logging.getLogger(__name__)


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
                    "%s.close: error during shutdown",
                    type(self).__name__,
                    exc_info=True,
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
            logger.warning(
                "%s: connection lost mid-frame: %s", type(self).__name__, exc
            )
            return None
        if data is None:
            return None
        return msgpack.unpackb(data, raw=False)
