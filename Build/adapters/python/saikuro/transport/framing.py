"""
Frame codec for length-prefixed message framing.
"""

from __future__ import annotations

import asyncio
import struct
from typing import Optional


_LENGTH_HEADER = struct.Struct(">I")  # big-endian uint32
_MAX_FRAME_SIZE = 16 * 1024 * 1024  # 16 MiB, which matches the Rust framing codec


def _check_frame_size(data: bytes) -> None:
    if len(data) > _MAX_FRAME_SIZE:
        raise ValueError(
            f"frame {len(data)} bytes exceeds maximum {_MAX_FRAME_SIZE} bytes"
        )


async def _send_frame(writer: asyncio.StreamWriter, data: bytes) -> None:
    _check_frame_size(data)
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
            # Clean EOF: the peer closed the connection gracefully.
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
