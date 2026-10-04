"""
Tests for the server-side WebSocket transport and its listener.
"""

from __future__ import annotations

import pytest

from saikuro.transport import (
    WebSocketListener,
    WebSocketServerTransport,
    WebSocketTransport,
)


@pytest.mark.asyncio
async def test_bind_reports_a_bound_port():
    listener = await WebSocketListener.bind(host="127.0.0.1", port=0)
    try:
        assert listener.port > 0
    finally:
        await listener.close()


@pytest.mark.asyncio
async def test_accepts_a_client_and_relays_frames_both_ways():
    listener = await WebSocketListener.bind(host="127.0.0.1", port=0)
    try:
        client = WebSocketTransport(f"ws://127.0.0.1:{listener.port}")
        await client.connect()
        server = await listener.accept()
        assert isinstance(server, WebSocketServerTransport)
        try:
            payload = {"type": "call", "id": "abc", "args": [1, 2]}
            await client.send(payload)
            assert await server.recv() == payload

            reply = {"type": "response", "id": "abc", "result": 3}
            await server.send(reply)
            assert await client.recv() == reply
        finally:
            await client.close()
            await server.close()
    finally:
        await listener.close()


@pytest.mark.asyncio
async def test_queues_a_connection_that_arrives_before_accept():
    listener = await WebSocketListener.bind(host="127.0.0.1", port=0)
    try:
        client = WebSocketTransport(f"ws://127.0.0.1:{listener.port}")
        await client.connect()
        # Accepted only after the client is already connected.
        server = await listener.accept()
        assert isinstance(server, WebSocketServerTransport)
        await client.close()
        await server.close()
    finally:
        await listener.close()


@pytest.mark.asyncio
async def test_recv_returns_none_when_client_closes():
    listener = await WebSocketListener.bind(host="127.0.0.1", port=0)
    try:
        client = WebSocketTransport(f"ws://127.0.0.1:{listener.port}")
        await client.connect()
        server = await listener.accept()
        try:
            await client.close()
            assert await server.recv() is None
        finally:
            await server.close()
    finally:
        await listener.close()


@pytest.mark.asyncio
async def test_accept_after_close_raises():
    listener = await WebSocketListener.bind(host="127.0.0.1", port=0)
    await listener.close()
    with pytest.raises(RuntimeError, match="closed"):
        await listener.accept()


@pytest.mark.asyncio
async def test_close_is_idempotent():
    listener = await WebSocketListener.bind(host="127.0.0.1", port=0)
    await listener.close()
    await listener.close()  # should not raise


@pytest.mark.asyncio
async def test_server_transport_send_after_close_raises():
    listener = await WebSocketListener.bind(host="127.0.0.1", port=0)
    try:
        client = WebSocketTransport(f"ws://127.0.0.1:{listener.port}")
        await client.connect()
        server = await listener.accept()
        await server.close()
        with pytest.raises(RuntimeError, match="not connected"):
            await server.send({"x": 1})
        await client.close()
    finally:
        await listener.close()
