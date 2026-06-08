"""
TCP transport.
"""

import asyncio

from saikuro.transport.stream import _StreamTransport


class TcpTransport(_StreamTransport):
    """Connects to a Saikuro runtime over TCP."""

    def __init__(self, host: str, port: int) -> None:
        super().__init__()
        self._host = host
        self._port = port

    async def connect(self) -> None:
        self._reader, self._writer = await asyncio.wait_for(
            asyncio.open_connection(self._host, self._port),
            timeout=10.0,
        )
