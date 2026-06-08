"""
Unix socket transport.
"""

import asyncio

from saikuro.transport.stream import _StreamTransport


class UnixSocketTransport(_StreamTransport):
    """Connects to a Saikuro runtime over a Unix domain socket."""

    def __init__(self, path: str) -> None:
        super().__init__()
        self._path = path

    async def connect(self) -> None:
        self._reader, self._writer = await asyncio.open_unix_connection(self._path)
