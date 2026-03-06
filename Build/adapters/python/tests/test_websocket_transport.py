"""
Tests for WebSocketTransport
"""

from __future__ import annotations

from typing import Any

import msgpack
import pytest
from websockets.asyncio.server import serve as ws_serve
from websockets.asyncio.server import ServerConnection

from saikuro.transport import WebSocketTransport, make_transport


#  Test server helpers


async def _echo_handler(ws: ServerConnection) -> None:
    """Echo each binary message straight back."""
    try:
        async for message in ws:
            if isinstance(message, (bytes, bytearray)):
                await ws.send(bytes(message))
    except Exception:
        pass


async def _close_on_receive_handler(ws: ServerConnection) -> None:
    """Close the connection after receiving the first message."""
    try:
        async for _message in ws:
            await ws.close()
            return
    except Exception:
        pass


async def _send_text_handler(ws: ServerConnection) -> None:
    """Send a text frame followed by a valid binary frame."""
    await ws.send("this is text, not binary")
    payload = msgpack.packb({"after_text": True}, use_bin_type=True)
    await ws.send(payload)


async def _send_and_close_handler(ws: ServerConnection) -> None:
    """Send one binary message then close."""
    payload = msgpack.packb({"hello": "from server"}, use_bin_type=True)
    await ws.send(payload)
    await ws.close()


def _free_port() -> int:
    """Return a free TCP port on localhost."""
    import socket

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


#  Fixtures


async def _start_server(handler: Any) -> tuple[Any, int]:
    """Start a websockets server with the given handler, return (server, port)."""
    port = _free_port()
    server = await ws_serve(handler, "127.0.0.1", port, compression=None)
    return server, port


#  Tests


@pytest.mark.asyncio
async def test_connect_and_close():
    server, port = await _start_server(_echo_handler)
    async with server:
        transport = WebSocketTransport(f"ws://127.0.0.1:{port}")
        await transport.connect()
        assert transport._ws is not None
        await transport.close()
        assert transport._ws is None


@pytest.mark.asyncio
async def test_send_and_recv_round_trip():
    server, port = await _start_server(_echo_handler)
    async with server:
        transport = WebSocketTransport(f"ws://127.0.0.1:{port}")
        await transport.connect()
        try:
            payload = {
                "type": "call",
                "id": "abc",
                "target": "math.add",
                "args": [1, 2],
            }
            await transport.send(payload)
            received = await transport.recv()
            assert received == payload
        finally:
            await transport.close()


@pytest.mark.asyncio
async def test_multiple_messages_in_order():
    server, port = await _start_server(_echo_handler)
    async with server:
        transport = WebSocketTransport(f"ws://127.0.0.1:{port}")
        await transport.connect()
        try:
            messages = [{"n": i} for i in range(5)]
            for msg in messages:
                await transport.send(msg)
            received = [await transport.recv() for _ in range(5)]
            assert received == messages
        finally:
            await transport.close()


@pytest.mark.asyncio
async def test_recv_returns_none_when_server_closes():
    server, port = await _start_server(_send_and_close_handler)
    async with server:
        transport = WebSocketTransport(f"ws://127.0.0.1:{port}")
        await transport.connect()
        try:
            # Server sends one message then closes.
            first = await transport.recv()
            assert first == {"hello": "from server"}
            # Next recv should return None (connection closed).
            second = await transport.recv()
            assert second is None
        finally:
            await transport.close()


@pytest.mark.asyncio
async def test_send_raises_when_not_connected():
    transport = WebSocketTransport("ws://127.0.0.1:9999")
    with pytest.raises(RuntimeError, match="not connected"):
        await transport.send({"x": 1})


@pytest.mark.asyncio
async def test_recv_returns_none_when_closed():
    server, port = await _start_server(_echo_handler)
    async with server:
        transport = WebSocketTransport(f"ws://127.0.0.1:{port}")
        await transport.connect()
        await transport.close()
        result = await transport.recv()
        assert result is None


@pytest.mark.asyncio
async def test_close_is_idempotent():
    server, port = await _start_server(_echo_handler)
    async with server:
        transport = WebSocketTransport(f"ws://127.0.0.1:{port}")
        await transport.connect()
        await transport.close()
        await transport.close()  # should not raise


@pytest.mark.asyncio
async def test_context_manager():
    server, port = await _start_server(_echo_handler)
    async with server:
        transport = WebSocketTransport(f"ws://127.0.0.1:{port}")
        async with transport:
            await transport.send({"ping": True})
            result = await transport.recv()
            assert result == {"ping": True}
        # After context exit, transport should be closed.
        with pytest.raises(RuntimeError, match="not connected"):
            await transport.send({"after": True})


@pytest.mark.asyncio
async def test_connect_failure_raises_runtime_error():
    # No server listening on this port.
    transport = WebSocketTransport("ws://127.0.0.1:1", open_timeout=0.5)
    with pytest.raises(RuntimeError, match="failed to connect"):
        await transport.connect()


@pytest.mark.asyncio
async def test_invalid_msgpack_frame_returns_none():
    """
    A binary WebSocket frame that is not valid MessagePack is a protocol error.
    recv() must return None (and log a warning) rather than raise.

    Note: websockets.recv(decode=False) returns both text and binary frames as
    bytes, so a text frame arriving on a binary-only protocol is indistinguishable
    from any other malformed payload :  both return None.
    """
    server, port = await _start_server(_send_text_handler)
    async with server:
        transport = WebSocketTransport(f"ws://127.0.0.1:{port}")
        await transport.connect()
        try:
            # The text frame (sent as UTF-8 bytes by the server) is not valid
            # MessagePack, so recv() returns None.
            result = await transport.recv()
            assert result is None
        finally:
            await transport.close()


@pytest.mark.asyncio
async def test_large_message_round_trip():
    server, port = await _start_server(_echo_handler)
    async with server:
        #  KiB of data :  well within the 16 MiB limit.
        big_payload = {"data": "x" * 100_000}
        transport = WebSocketTransport(f"ws://127.0.0.1:{port}")
        await transport.connect()
        try:
            await transport.send(big_payload)
            received = await transport.recv()
            assert received == big_payload
        finally:
            await transport.close()


def test_make_transport_ws_scheme():
    from saikuro.transport import WebSocketTransport as WS

    t = make_transport("ws://localhost:8765/saikuro")
    assert isinstance(t, WS)


def test_make_transport_wss_scheme():
    from saikuro.transport import WebSocketTransport as WS

    t = make_transport("wss://example.com/saikuro")
    assert isinstance(t, WS)


def test_make_transport_unknown_scheme_raises():
    with pytest.raises(ValueError, match="unsupported transport address"):
        make_transport("ftp://nope")
