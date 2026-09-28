"""
Tests for the provider/client schema-announce handshake.

``SaikuroProvider.serve_on`` announces its schema as the first frame and then
blocks on ``recv()`` until the runtime acknowledges it. The client is the
runtime in these tests, so it has to send that ack. Without it the provider
consumes the next request frame while looking for the ack, and that first call
never gets a response.
"""

import asyncio
import contextlib

from saikuro.client import SaikuroClient
from saikuro.envelope import Envelope
from saikuro.provider import SaikuroProvider
from saikuro.transport import InMemoryTransport

#  Bound on calls that must not need the handshake to be skipped. Comfortably
#  above the provider's announce round trip, far below any real timeout.
PROMPT_TIMEOUT = 2.0


def math_provider() -> SaikuroProvider:
    """A provider with one handler, enough to prove the serve loop is live."""
    provider = SaikuroProvider("math")

    @provider.register("add")
    def add(a: float, b: float) -> float:
        return a + b

    return provider


async def collect_acks(transport: InMemoryTransport, acks: asyncio.Queue[dict]) -> None:
    """Forward every response-shaped frame (``ok`` present) into ``acks``."""
    while True:
        raw = await transport.recv()
        if raw is None:
            return
        if raw.get("ok") is not None:
            await acks.put(raw)


#  Ack frame


async def test_client_acks_schema_announce() -> None:
    client_side, provider_side = InMemoryTransport.pair()

    client = SaikuroClient.from_transport(client_side)
    await client._connect()

    acks: asyncio.Queue[dict] = asyncio.Queue()
    drain = asyncio.create_task(collect_acks(provider_side, acks))

    announce = Envelope.make_announce({"namespaces": {}})
    await provider_side.send(announce.to_msgpack_dict())

    ack = await asyncio.wait_for(acks.get(), PROMPT_TIMEOUT)
    assert ack == {"id": announce.id, "ok": True}

    drain.cancel()
    with contextlib.suppress(asyncio.CancelledError):
        await drain
    await client.close()


async def test_client_survives_an_announce_it_cannot_ack() -> None:
    client_side, provider_side = InMemoryTransport.pair()

    client = SaikuroClient.from_transport(client_side)
    await client._connect()

    acks: asyncio.Queue[dict] = asyncio.Queue()
    drain = asyncio.create_task(collect_acks(provider_side, acks))

    # An announce carrying no id cannot be acked; the client must log and carry
    # on rather than drop the connection.
    await provider_side.send(
        {"version": 1, "type": "announce", "target": "$saikuro.announce"}
    )

    # A well-formed announce right after it still gets acked.
    announce = Envelope.make_announce({"namespaces": {}})
    await provider_side.send(announce.to_msgpack_dict())

    ack = await asyncio.wait_for(acks.get(), PROMPT_TIMEOUT)
    assert ack == {"id": announce.id, "ok": True}

    drain.cancel()
    with contextlib.suppress(asyncio.CancelledError):
        await drain
    await client.close()


#  End-to-end over the real serve loop


async def test_serve_on_completes_handshake_and_answers_first_call() -> None:
    client_side, provider_side = InMemoryTransport.pair()

    client = SaikuroClient.from_transport(client_side)
    await client._connect()

    serve_task = asyncio.create_task(math_provider().serve_on(provider_side))

    # The very first call lands while the announce ack is still in flight.
    result = await client.call("math.add", [41.0, 1.0], timeout=PROMPT_TIMEOUT)
    assert result == 42.0

    serve_task.cancel()
    with contextlib.suppress(asyncio.CancelledError):
        await serve_task
    await client.close()
