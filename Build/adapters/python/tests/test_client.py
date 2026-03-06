"""
Tests for SaikuroClient
"""

import asyncio
import pytest
from saikuro.client import SaikuroClient
from saikuro.provider import SaikuroProvider
from saikuro.envelope import Envelope, InvocationType, ResourceHandle
from saikuro.error import SaikuroError, TransportError
from saikuro.transport import InMemoryTransport


#  Harness


class Harness:
    def __init__(
        self,
        client: SaikuroClient,
        provider: SaikuroProvider,
        serve_task: asyncio.Task,
        client_transport: InMemoryTransport,
    ):
        self.client = client
        self.provider = provider
        self._serve_task = serve_task
        self._client_transport = client_transport

    async def teardown(self):
        await self.client.close()
        self._serve_task.cancel()
        try:
            await self._serve_task
        except (asyncio.CancelledError, Exception):
            pass


async def make_harness(namespace: str = "test") -> Harness:
    """
    Wire a SaikuroClient and SaikuroProvider together without the announce
    handshake so tests are instant.
    """
    client_transport, provider_transport = InMemoryTransport.pair()
    provider = SaikuroProvider(namespace)
    client = SaikuroClient.from_transport(client_transport)
    await client._connect()

    async def serve_loop():
        while True:
            raw = await provider_transport.recv()
            if raw is None:
                break
            try:
                envelope = Envelope.from_msgpack_dict(raw)
            except Exception:
                continue
            asyncio.ensure_future(provider._dispatch(envelope, provider_transport))

    serve_task = asyncio.ensure_future(serve_loop())
    return Harness(client, provider, serve_task, client_transport)


#  call()


class TestClientCall:
    @pytest.mark.asyncio
    async def test_sync_handler_result(self):
        h = await make_harness()
        h.provider.register_function("add", lambda a, b: a + b)
        result = await h.client.call("test.add", [3, 4])
        assert result == 7
        await h.teardown()

    @pytest.mark.asyncio
    async def test_async_handler_result(self):
        h = await make_harness()

        async def greet(name):
            return f"Hello, {name}"

        h.provider.register_function("greet", greet)
        result = await h.client.call("test.greet", ["world"])
        assert result == "Hello, world"
        await h.teardown()

    @pytest.mark.asyncio
    async def test_raises_saikuro_error_on_handler_exception(self):
        h = await make_harness()
        h.provider.register_function(
            "boom", lambda: (_ for _ in ()).throw(RuntimeError("exploded"))
        )
        with pytest.raises(SaikuroError):
            await h.client.call("test.boom", [])
        await h.teardown()

    @pytest.mark.asyncio
    async def test_error_message_propagated(self):
        h = await make_harness()
        h.provider.register_function(
            "boom", lambda: (_ for _ in ()).throw(RuntimeError("exploded"))
        )
        with pytest.raises(SaikuroError) as exc_info:
            await h.client.call("test.boom", [])
        assert "exploded" in str(exc_info.value)
        await h.teardown()

    @pytest.mark.asyncio
    async def test_raises_on_missing_function(self):
        h = await make_harness()
        with pytest.raises(SaikuroError):
            await h.client.call("test.missing_fn", [])
        await h.teardown()

    @pytest.mark.asyncio
    async def test_passes_args_correctly(self):
        h = await make_harness()
        h.provider.register_function("echo", lambda *args: list(args))
        result = await h.client.call("test.echo", [None, "hi", [1, 2]])
        assert result == [None, "hi", [1, 2]]
        await h.teardown()

    @pytest.mark.asyncio
    async def test_null_result(self):
        h = await make_harness()
        h.provider.register_function("nil", lambda: None)
        result = await h.client.call("test.nil", [])
        assert result is None
        await h.teardown()


#  call() timeout


class TestClientCallTimeout:
    @pytest.mark.asyncio
    async def test_timeout_raises_asyncio_timeout_error(self):
        # No provider :  client will never receive a response.
        a, _ = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()
        with pytest.raises(asyncio.TimeoutError):
            await client.call("nowhere.fn", [], timeout=0.01)
        await client.close()


#  cast()


class TestClientCast:
    @pytest.mark.asyncio
    async def test_cast_resolves_without_error(self):
        a, _ = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()
        await client.cast("events.fire", [{"type": "click"}])
        await client.close()

    @pytest.mark.asyncio
    async def test_cast_delivers_envelope_to_transport(self):
        a, b = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()
        await client.cast("test.track", [{"ev": "view"}])
        raw = await b.recv()
        assert raw is not None
        assert raw["type"] == "cast"
        assert raw["target"] == "test.track"
        await client.close()


#  resource()


class TestClientResource:
    @pytest.mark.asyncio
    async def test_decodes_resource_handle(self):
        h = await make_harness()
        handle = ResourceHandle(id="res-42", mime_type="text/plain", size=100)
        h.provider.register_function("open", lambda: handle.to_dict())
        result = await h.client.resource("test.open", [])
        assert isinstance(result, ResourceHandle)
        assert result.id == "res-42"
        assert result.mime_type == "text/plain"
        await h.teardown()

    @pytest.mark.asyncio
    async def test_raises_on_non_dict_result(self):
        h = await make_harness()
        h.provider.register_function("open", lambda: "not-a-handle")
        with pytest.raises((ValueError, SaikuroError)):
            await h.client.resource("test.open", [])
        await h.teardown()

    @pytest.mark.asyncio
    async def test_raises_on_provider_error(self):
        h = await make_harness()
        h.provider.register_function(
            "open", lambda: (_ for _ in ()).throw(FileNotFoundError("missing"))
        )
        with pytest.raises(SaikuroError):
            await h.client.resource("test.open", [])
        await h.teardown()


#  stream()


class TestClientStream:
    @pytest.mark.asyncio
    async def test_yields_all_items(self):
        h = await make_harness()

        @h.provider.register("count")
        async def count():
            yield 1
            yield 2
            yield 3

        stream = await h.client.stream("test.count", [])
        items = []
        async for item in stream:
            items.append(item)
        assert items == [1, 2, 3]
        await h.teardown()

    @pytest.mark.asyncio
    async def test_empty_generator_produces_no_items(self):
        h = await make_harness()

        @h.provider.register("empty")
        async def empty():
            return
            yield  # make it an async generator

        stream = await h.client.stream("test.empty", [])
        items = []
        async for item in stream:
            items.append(item)
        assert items == []
        await h.teardown()

    @pytest.mark.asyncio
    async def test_raises_on_mid_stream_exception(self):
        h = await make_harness()

        @h.provider.register("fail_after_one")
        async def fail_after_one():
            yield 10
            raise ValueError("mid-stream failure")

        stream = await h.client.stream("test.fail_after_one", [])
        items = []
        with pytest.raises(SaikuroError):
            async for item in stream:
                items.append(item)
        assert items == [10]
        await h.teardown()


#  channel()


class TestClientChannel:
    @pytest.mark.asyncio
    async def test_channel_receives_items_from_provider(self):
        h = await make_harness()

        @h.provider.register("echo_ch")
        async def echo_ch():
            yield "alpha"
            yield "beta"

        ch = await h.client.channel("test.echo_ch", [])
        received = []
        async for item in ch:
            received.append(item)
        assert received == ["alpha", "beta"]
        await h.teardown()

    @pytest.mark.asyncio
    async def test_channel_has_invocation_id(self):
        h = await make_harness()

        @h.provider.register("ch")
        async def ch():
            yield "x"

        channel = await h.client.channel("test.ch", [])
        assert isinstance(channel.invocation_id, str)
        assert len(channel.invocation_id) > 0
        async for _ in channel:
            pass
        await h.teardown()


#  close() teardown


class TestClientCloseTeardown:
    @pytest.mark.asyncio
    async def test_close_rejects_pending_calls_with_transport_error(self):
        a, _ = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()

        call_task = asyncio.create_task(client.call("nowhere.fn", []))
        await asyncio.sleep(0)  # let task register the pending call
        await client.close()

        with pytest.raises((TransportError, asyncio.CancelledError)):
            await call_task


#  concurrent calls


class TestClientConcurrentCalls:
    @pytest.mark.asyncio
    async def test_concurrent_calls_resolve_independently(self):
        h = await make_harness()
        h.provider.register_function("double", lambda n: n * 2)

        results = await asyncio.gather(
            h.client.call("test.double", [1]),
            h.client.call("test.double", [2]),
            h.client.call("test.double", [3]),
            h.client.call("test.double", [4]),
        )
        assert sorted(results) == [2, 4, 6, 8]
        await h.teardown()


#  log()


class TestClientLog:
    @pytest.mark.asyncio
    async def test_log_resolves_without_error(self):
        a, _ = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()
        from saikuro.envelope import LogLevel

        await client.log(LogLevel.INFO, "test", "hello", {"k": "v"})
        await client.close()

    @pytest.mark.asyncio
    async def test_log_delivers_log_envelope_to_transport(self):
        a, b = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()
        from saikuro.envelope import LogLevel

        await client.log(
            LogLevel.WARN, "my.logger", "something happened", {"detail": 42}
        )
        raw = await b.recv()
        assert raw is not None
        assert raw["type"] == "log"
        assert raw["target"] == "$log"
        log_record = raw["args"][0]
        assert log_record["level"] == "warn"
        assert log_record["msg"] == "something happened"
        await client.close()


#  batch()


def _make_batch_server(transport: InMemoryTransport, handlers: dict) -> asyncio.Task:
    """Spawn a task that processes batch envelopes by dispatching each item
    independently to the provided function map and returning an Array result.

    `handlers` maps function-name (last segment of target) to a callable.
    """

    async def serve():
        while True:
            raw = await transport.recv()
            if raw is None:
                break
            try:
                envelope = Envelope.from_msgpack_dict(raw)
            except Exception:
                continue
            if envelope.invocation_type == InvocationType.BATCH:
                results = []
                for item in envelope.batch_items or []:
                    fn_name = item.target.split(".")[-1]
                    handler = handlers.get(fn_name)
                    if handler is None:
                        results.append(None)
                    else:
                        try:
                            result = handler(*item.args)
                        except Exception:
                            result = None
                        results.append(result)
                await transport.send(
                    {
                        "id": envelope.id,
                        "ok": True,
                        "result": results,
                    }
                )

    return asyncio.ensure_future(serve())


class TestClientBatch:
    @pytest.mark.asyncio
    async def test_batch_returns_ordered_results(self):
        a, b = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()
        task = _make_batch_server(
            b,
            {
                "add": lambda x, y: x + y,
                "mul": lambda x, y: x * y,
            },
        )

        results = await client.batch(
            [
                ("math.add", [2, 3]),
                ("math.mul", [4, 5]),
            ]
        )
        assert results == [5, 20]

        await client.close()
        task.cancel()

    @pytest.mark.asyncio
    async def test_batch_single_item(self):
        a, b = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()
        task = _make_batch_server(b, {"echo": lambda v: v})

        results = await client.batch([("svc.echo", ["hello"])])
        assert results == ["hello"]

        await client.close()
        task.cancel()

    @pytest.mark.asyncio
    async def test_batch_with_capability_tuple(self):
        a, b = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()

        # Capture the raw envelope sent to inspect the batch_items.
        captured: list = []

        async def capture():
            raw = await b.recv()
            if raw:
                captured.append(raw)
                await b.send({"id": raw["id"], "ok": True, "result": [42]})

        cap_task = asyncio.ensure_future(capture())

        results = await client.batch([("svc.fn", [1], "my.cap")])
        assert results == [42]

        # Verify the batch item carried the capability.
        assert len(captured) == 1
        items = captured[0].get("batch_items", [])
        assert len(items) == 1
        assert items[0].get("capability") == "my.cap"

        await client.close()
        cap_task.cancel()

    @pytest.mark.asyncio
    async def test_batch_failed_item_returns_none(self):
        a, b = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()
        task = _make_batch_server(
            b,
            {
                "ok": lambda: "fine",
                # "missing" is not registered → None result
            },
        )

        results = await client.batch(
            [
                ("svc.ok", []),
                ("svc.missing", []),
            ]
        )
        assert results[0] == "fine"
        assert results[1] is None

        await client.close()
        task.cancel()

    @pytest.mark.asyncio
    async def test_batch_raises_saikuro_error_on_envelope_rejection(self):
        a, b = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()

        async def reject_server():
            raw = await b.recv()
            if raw:
                await b.send(
                    {
                        "id": raw["id"],
                        "ok": False,
                        "error": {
                            "code": "MalformedEnvelope",
                            "message": "empty batch",
                        },
                    }
                )

        task = asyncio.ensure_future(reject_server())
        with pytest.raises(SaikuroError):
            await client.batch([("svc.fn", [])])

        await client.close()
        task.cancel()

    @pytest.mark.asyncio
    async def test_batch_timeout_raises(self):
        a, _ = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()

        with pytest.raises(asyncio.TimeoutError):
            await client.batch([("svc.fn", [])], timeout=0.01)

        await client.close()

    @pytest.mark.asyncio
    async def test_batch_sends_batch_type_envelope(self):
        a, b = InMemoryTransport.pair()
        client = SaikuroClient.from_transport(a)
        await client._connect()

        # Read the raw frame without responding :  just inspect it.
        send_task = asyncio.ensure_future(
            client.batch([("ns.fn1", [1]), ("ns.fn2", [2])], timeout=0.5)
        )
        await asyncio.sleep(0)  # yield so the batch is sent
        raw = await b.recv()
        assert raw is not None
        assert raw["type"] == "batch"
        items = raw.get("batch_items", [])
        assert len(items) == 2
        assert items[0]["target"] == "ns.fn1"
        assert items[1]["target"] == "ns.fn2"

        # Send back a response to avoid the timeout.
        await b.send({"id": raw["id"], "ok": True, "result": [None, None]})
        await send_task
        await client.close()
