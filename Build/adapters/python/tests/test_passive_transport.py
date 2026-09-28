"""
Tests for the passive side of the stream-backed transports.
"""

import asyncio
import contextlib
import tempfile
from pathlib import Path

import pytest

from saikuro.transport import TcpTransport, UnixSocketTransport


@contextlib.asynccontextmanager
async def _tcp_pair():
    """Yield (client transport, provider-side transport) over one TCP connection."""
    accepted: asyncio.Future[tuple[asyncio.StreamReader, asyncio.StreamWriter]] = (
        asyncio.get_running_loop().create_future()
    )

    async def on_connection(
        reader: asyncio.StreamReader, writer: asyncio.StreamWriter
    ) -> None:
        if not accepted.done():
            accepted.set_result((reader, writer))

    server = await asyncio.start_server(on_connection, "127.0.0.1", 0)
    port = (server.sockets or ())[0].getsockname()[1]

    client: TcpTransport | None = None
    provider_side: TcpTransport | None = None
    try:
        client = TcpTransport("127.0.0.1", port)
        await client.connect()
        reader, writer = await accepted
        provider_side = TcpTransport.from_stream(reader, writer)
        yield client, provider_side
    finally:
        if client is not None:
            await client.close()
        if provider_side is not None:
            await provider_side.close()
        server.close()
        await server.wait_closed()


@contextlib.asynccontextmanager
async def _unix_pair():
    """Yield (client transport, provider-side transport) over one Unix socket."""
    accepted: asyncio.Future[tuple[asyncio.StreamReader, asyncio.StreamWriter]] = (
        asyncio.get_running_loop().create_future()
    )

    async def on_connection(
        reader: asyncio.StreamReader, writer: asyncio.StreamWriter
    ) -> None:
        if not accepted.done():
            accepted.set_result((reader, writer))

    with tempfile.TemporaryDirectory() as tmp:
        path = str(Path(tmp) / "saikuro.sock")
        server = await asyncio.start_unix_server(on_connection, path)

        client: UnixSocketTransport | None = None
        provider_side: UnixSocketTransport | None = None
        try:
            client = UnixSocketTransport(path)
            await client.connect()
            reader, writer = await accepted
            provider_side = UnixSocketTransport.from_stream(reader, writer)
            yield client, provider_side
        finally:
            if client is not None:
                await client.close()
            if provider_side is not None:
                await provider_side.close()
            server.close()
            await server.wait_closed()


@pytest.mark.asyncio
async def test_from_stream_adopts_an_accepted_tcp_connection():
    async with _tcp_pair() as (client, provider_side):
        assert provider_side.is_connected
        await client.send({"ping": 1})
        assert await provider_side.recv() == {"ping": 1}


@pytest.mark.asyncio
async def test_connect_on_adopted_tcp_transport_does_not_redial():
    async with _tcp_pair() as (client, provider_side):
        # An adopted transport has no dial target, so a redial would raise
        # rather than silently open a second socket. It must be a no-op.
        await provider_side.connect()
        assert provider_side.is_connected

        # The original session is still the live one.
        await client.send({"ping": 2})
        assert await provider_side.recv() == {"ping": 2}


@pytest.mark.asyncio
async def test_from_stream_adopts_an_accepted_unix_connection():
    async with _unix_pair() as (client, provider_side):
        assert provider_side.is_connected
        await client.send({"ping": 3})
        assert await provider_side.recv() == {"ping": 3}


@pytest.mark.asyncio
async def test_connect_on_adopted_unix_transport_does_not_redial():
    async with _unix_pair() as (client, provider_side):
        await provider_side.connect()
        assert provider_side.is_connected
        await client.send({"ping": 4})
        assert await provider_side.recv() == {"ping": 4}
