"""
Tests for SaikuroProvider
"""

import pytest
from saikuro.provider import SaikuroProvider
from saikuro.envelope import Envelope, InvocationType
from saikuro.error import SaikuroError
from saikuro.transport import InMemoryTransport


# Helpers


def make_env(
    itype: InvocationType, target: str, args: list, inv_id: str = "test-id"
) -> Envelope:
    return Envelope(
        version=1,
        invocation_type=itype,
        id=inv_id,
        target=target,
        args=args,
    )


async def collect_responses(transport: InMemoryTransport, n: int) -> list:
    results = []
    for _ in range(n):
        item = await transport.recv()
        if item is None:
            break
        results.append(item)
    return results


# namespace / register


class TestProviderNamespace:
    def test_namespace_property(self):
        p = SaikuroProvider("math")
        assert p.namespace == "math"


class TestProviderRegister:
    def test_register_decorator_returns_function(self):
        p = SaikuroProvider("test")

        @p.register("add")
        def add(a, b):
            return a + b

        assert callable(add)

    def test_register_function_imperative(self):
        p = SaikuroProvider("test")
        p.register_function("mul", lambda a, b: a * b)
        # function is in the handlers map
        assert "mul" in p._handlers

    @pytest.mark.asyncio
    async def test_register_function_is_callable_via_dispatch(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("test")
        p.register_function("double", lambda x: x * 2)
        await p._dispatch(make_env(InvocationType.CALL, "test.double", [5]), a)
        resp = await b.recv()
        assert resp is not None
        assert resp["ok"] is True
        assert resp["result"] == 10


# dispatch() sync handler


class TestDispatchSyncHandler:
    @pytest.mark.asyncio
    async def test_returns_correct_result(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("math")

        @p.register("add")
        def add(x, y):
            return x + y

        await p._dispatch(make_env(InvocationType.CALL, "math.add", [3, 4]), a)
        resp = await b.recv()
        assert resp == {"id": "test-id", "ok": True, "result": 7}

    @pytest.mark.asyncio
    async def test_uses_last_segment_of_target(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("math")
        p.register_function("mul", lambda x, y: x * y)
        await p._dispatch(make_env(InvocationType.CALL, "math.mul", [3, 5]), a)
        resp = await b.recv()
        assert resp is not None
        assert resp["result"] == 15

    @pytest.mark.asyncio
    async def test_null_result(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("test")
        p.register_function("nil", lambda: None)
        await p._dispatch(make_env(InvocationType.CALL, "test.nil", []), a)
        resp = await b.recv()
        assert resp is not None
        assert resp["ok"] is True
        assert resp["result"] is None


# dispatch() async handler


class TestDispatchAsyncHandler:
    @pytest.mark.asyncio
    async def test_awaits_async_handler(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("math")

        @p.register("async_add")
        async def async_add(x, y):
            return x + y

        await p._dispatch(make_env(InvocationType.CALL, "math.async_add", [10, 20]), a)
        resp = await b.recv()
        assert resp is not None
        assert resp["result"] == 30


# dispatch() stream handler


class TestDispatchStreamHandler:
    @pytest.mark.asyncio
    async def test_yields_items_with_seq_then_end(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("test")

        @p.register("count")
        async def count():
            yield "a"
            yield "b"
            yield "c"

        await p._dispatch(make_env(InvocationType.STREAM, "test.count", []), a)
        responses = await collect_responses(b, 4)

        assert responses[0] == {"id": "test-id", "ok": True, "result": "a", "seq": 0}
        assert responses[1] == {"id": "test-id", "ok": True, "result": "b", "seq": 1}
        assert responses[2] == {"id": "test-id", "ok": True, "result": "c", "seq": 2}
        assert responses[3]["stream_control"] == "end"
        assert responses[3]["ok"] is True

    @pytest.mark.asyncio
    async def test_empty_generator_sends_only_end(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("test")

        @p.register("empty")
        async def empty():
            return
            yield  # make it an async generator

        await p._dispatch(make_env(InvocationType.STREAM, "test.empty", []), a)
        responses = await collect_responses(b, 1)
        assert responses[0]["stream_control"] == "end"

    @pytest.mark.asyncio
    async def test_mid_stream_exception_sends_error_then_abort(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("test")

        @p.register("fail_stream")
        async def fail_stream():
            yield 42
            raise ValueError("mid-stream failure")

        await p._dispatch(make_env(InvocationType.STREAM, "test.fail_stream", []), a)
        responses = await collect_responses(b, 3)

        # First item
        assert responses[0]["ok"] is True
        assert responses[0]["result"] == 42
        # Error frame
        assert responses[1]["ok"] is False
        assert "mid-stream failure" in responses[1]["error"]["message"]
        # Abort sentinel
        assert responses[2]["stream_control"] == "abort"


# dispatch() error cases


class TestDispatchErrorCases:
    @pytest.mark.asyncio
    async def test_function_not_found(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("test")
        await p._dispatch(make_env(InvocationType.CALL, "test.missing", []), a)
        resp = await b.recv()
        assert resp is not None
        assert resp["ok"] is False
        assert resp["error"]["code"] == "FunctionNotFound"

    @pytest.mark.asyncio
    async def test_plain_exception_becomes_provider_error(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("test")
        p.register_function(
            "boom", lambda: (_ for _ in ()).throw(RuntimeError("exploded"))
        )
        await p._dispatch(make_env(InvocationType.CALL, "test.boom", []), a)
        resp = await b.recv()
        assert resp is not None
        assert resp["ok"] is False
        assert resp["error"]["code"] == "ProviderError"
        assert "exploded" in resp["error"]["message"]

    @pytest.mark.asyncio
    async def test_saikuro_error_preserves_code_and_message(self):
        a, b = InMemoryTransport.pair()
        p = SaikuroProvider("test")

        def deny():
            raise SaikuroError(code="CapabilityDenied", message="access denied")

        p.register_function("deny", deny)
        await p._dispatch(make_env(InvocationType.CALL, "test.deny", []), a)
        resp = await b.recv()
        assert resp is not None
        assert resp["ok"] is False
        assert resp["error"]["code"] == "CapabilityDenied"
        assert "access denied" in resp["error"]["message"]


# schema_dict()


class TestSchemaDict:
    def test_version_is_1(self):
        p = SaikuroProvider("math")
        assert p.schema_dict()["version"] == 1

    def test_namespace_present(self):
        p = SaikuroProvider("math")
        p.register_function("add", lambda a, b: a + b)
        assert "math" in p.schema_dict()["namespaces"]

    def test_registered_functions_present(self):
        p = SaikuroProvider("math")
        p.register_function("add", lambda a, b: a + b)
        p.register_function("sub", lambda a, b: a - b)
        fns = p.schema_dict()["namespaces"]["math"]["functions"]
        assert "add" in fns
        assert "sub" in fns

    def test_empty_functions_when_nothing_registered(self):
        p = SaikuroProvider("empty")
        fns = p.schema_dict()["namespaces"]["empty"]["functions"]
        assert fns == {}

    def test_doc_stored_when_provided(self):
        p = SaikuroProvider("math")
        p.register_function("add", lambda a, b: a + b, doc="adds two numbers")
        fn = p.schema_dict()["namespaces"]["math"]["functions"]["add"]
        assert fn.get("doc") == "adds two numbers"

    def test_capabilities_stored(self):
        p = SaikuroProvider("math")
        p.register_function("secret", lambda: 0, capabilities=["admin"])
        fn = p.schema_dict()["namespaces"]["math"]["functions"]["secret"]
        assert fn["capabilities"] == ["admin"]
