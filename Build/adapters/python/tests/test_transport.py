"""
Tests for InMemoryTransport
"""

import asyncio
import pytest
from saikuro.transport import InMemoryTransport


@pytest.mark.asyncio
async def test_pair_returns_two_distinct_instances():
    a, b = InMemoryTransport.pair()
    assert a is not b


@pytest.mark.asyncio
async def test_connect_is_noop():
    a, _ = InMemoryTransport.pair()
    await a.connect()  # should not raise


@pytest.mark.asyncio
async def test_send_delivers_to_peer():
    a, b = InMemoryTransport.pair()
    msg = {"hello": "world"}
    await a.send(msg)
    received = await b.recv()
    assert received == msg


@pytest.mark.asyncio
async def test_send_other_direction():
    a, b = InMemoryTransport.pair()
    await b.send({"reply": True})
    received = await a.recv()
    assert received == {"reply": True}


@pytest.mark.asyncio
async def test_messages_delivered_in_order():
    a, b = InMemoryTransport.pair()
    for i in range(5):
        await a.send({"n": i})
    results = []
    for _ in range(5):
        results.append((await b.recv())["n"])
    assert results == list(range(5))


@pytest.mark.asyncio
async def test_recv_returns_none_after_close():
    a, b = InMemoryTransport.pair()
    await b.close()
    result = await b.recv()
    assert result is None


@pytest.mark.asyncio
async def test_close_unblocks_recv():
    a, b = InMemoryTransport.pair()
    recv_task = asyncio.create_task(b.recv())
    await asyncio.sleep(0)  # let task start waiting
    await b.close()
    result = await recv_task
    assert result is None


@pytest.mark.asyncio
async def test_send_after_close_raises():
    a, b = InMemoryTransport.pair()
    await a.close()
    with pytest.raises(RuntimeError, match="closed"):
        await a.send({"x": 1})


@pytest.mark.asyncio
async def test_close_is_idempotent():
    a, _ = InMemoryTransport.pair()
    await a.close()
    await a.close()  # second close should not raise


@pytest.mark.asyncio
async def test_recv_on_closed_returns_none_immediately():
    a, b = InMemoryTransport.pair()
    await a.close()
    result = await a.recv()
    assert result is None


@pytest.mark.asyncio
async def test_context_manager_connects_and_closes():
    a, b = InMemoryTransport.pair()
    async with a:
        await a.send({"ping": True})
    # After exiting the context, a should be closed.
    with pytest.raises(RuntimeError):
        await a.send({"more": True})
