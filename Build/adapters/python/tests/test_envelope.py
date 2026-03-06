"""
Tests for envelope types and factory methods
"""

import pytest
from saikuro.envelope import (
    Envelope,
    InvocationType,
    LogLevel,
    LogRecord,
    ResourceHandle,
    ResponseEnvelope,
    StreamControl,
)


# InvocationType values


class TestInvocationType:
    def test_call_value(self):
        assert InvocationType.CALL.value == "call"

    def test_cast_value(self):
        assert InvocationType.CAST.value == "cast"

    def test_stream_value(self):
        assert InvocationType.STREAM.value == "stream"

    def test_channel_value(self):
        assert InvocationType.CHANNEL.value == "channel"

    def test_resource_value(self):
        assert InvocationType.RESOURCE.value == "resource"

    def test_announce_value(self):
        assert InvocationType.ANNOUNCE.value == "announce"

    def test_log_value(self):
        assert InvocationType.LOG.value == "log"


# Envelope factories


class TestEnvelopeFactories:
    def test_make_call_type(self):
        env = Envelope.make_call("math.add", [1, 2])
        assert env.invocation_type == InvocationType.CALL

    def test_make_call_target_and_args(self):
        env = Envelope.make_call("math.add", [1, 2])
        assert env.target == "math.add"
        assert env.args == [1, 2]

    def test_make_call_version(self):
        env = Envelope.make_call("fn", [])
        assert env.version == 1

    def test_make_call_unique_ids(self):
        a = Envelope.make_call("fn", [])
        b = Envelope.make_call("fn", [])
        assert a.id != b.id

    def test_make_call_capability(self):
        env = Envelope.make_call("fn", [], capability="tok-123")
        assert env.capability == "tok-123"

    def test_make_call_no_capability(self):
        env = Envelope.make_call("fn", [])
        assert env.capability is None

    def test_make_cast_type(self):
        env = Envelope.make_cast("log.info", ["hello"])
        assert env.invocation_type == InvocationType.CAST

    def test_make_stream_open_type(self):
        env = Envelope.make_stream_open("events.sub", [])
        assert env.invocation_type == InvocationType.STREAM

    def test_make_channel_open_type(self):
        env = Envelope.make_channel_open("chat.open", [])
        assert env.invocation_type == InvocationType.CHANNEL

    def test_make_resource_type(self):
        env = Envelope.make_resource("files.open", ["/tmp/x"])
        assert env.invocation_type == InvocationType.RESOURCE

    def test_make_resource_capability(self):
        env = Envelope.make_resource("files.open", [], capability="cap-abc")
        assert env.capability == "cap-abc"

    def test_make_announce_type(self):
        env = Envelope.make_announce({"version": 1, "namespaces": {}, "types": {}})
        assert env.invocation_type == InvocationType.ANNOUNCE

    def test_make_announce_target(self):
        env = Envelope.make_announce({})
        assert env.target == "$saikuro.announce"

    def test_make_announce_schema_in_args(self):
        schema = {"version": 1, "namespaces": {}, "types": {}}
        env = Envelope.make_announce(schema)
        assert env.args[0] == schema


# to_msgpack_dict / from_msgpack_dict round-trip


class TestEnvelopeSerialisation:
    def test_call_round_trip(self):
        env = Envelope.make_call("math.add", [3, 4])
        d = env.to_msgpack_dict()
        restored = Envelope.from_msgpack_dict(d)
        assert restored.invocation_type == InvocationType.CALL
        assert restored.target == "math.add"
        assert restored.args == [3, 4]
        assert restored.id == env.id

    def test_capability_preserved(self):
        env = Envelope.make_call("fn", [], capability="tok")
        d = env.to_msgpack_dict()
        restored = Envelope.from_msgpack_dict(d)
        assert restored.capability == "tok"

    def test_no_capability_when_absent(self):
        env = Envelope.make_call("fn", [])
        d = env.to_msgpack_dict()
        assert "capability" not in d

    def test_stream_control_round_trip(self):
        env = Envelope.make_stream_open("s", [])
        env.stream_control = StreamControl.END
        d = env.to_msgpack_dict()
        assert d["stream_control"] == "end"
        restored = Envelope.from_msgpack_dict(d)
        assert restored.stream_control == StreamControl.END

    def test_seq_round_trip(self):
        env = Envelope.make_call("fn", [])
        env.seq = 5
        d = env.to_msgpack_dict()
        assert d["seq"] == 5
        restored = Envelope.from_msgpack_dict(d)
        assert restored.seq == 5


# ResponseEnvelope


class TestResponseEnvelope:
    def test_ok_result(self):
        resp = ResponseEnvelope.from_msgpack_dict({"id": "x", "ok": True, "result": 42})
        assert resp.ok is True
        assert resp.result == 42
        assert resp.id == "x"

    def test_error_response(self):
        resp = ResponseEnvelope.from_msgpack_dict(
            {
                "id": "y",
                "ok": False,
                "error": {"code": "FunctionNotFound", "message": "not found"},
            }
        )
        assert resp.ok is False
        assert resp.error is not None
        assert resp.error["code"] == "FunctionNotFound"

    def test_stream_end(self):
        resp = ResponseEnvelope.from_msgpack_dict(
            {
                "id": "z",
                "ok": True,
                "stream_control": "end",
            }
        )
        assert resp.is_stream_end is True

    def test_stream_abort_is_end(self):
        resp = ResponseEnvelope.from_msgpack_dict(
            {
                "id": "z",
                "ok": False,
                "stream_control": "abort",
            }
        )
        assert resp.is_stream_end is True

    def test_non_end_control_is_not_end(self):
        resp = ResponseEnvelope.from_msgpack_dict(
            {
                "id": "z",
                "ok": True,
                "stream_control": "pause",
            }
        )
        assert resp.is_stream_end is False

    def test_no_stream_control_is_not_end(self):
        resp = ResponseEnvelope.from_msgpack_dict({"id": "z", "ok": True, "result": 1})
        assert resp.is_stream_end is False


# ResourceHandle


class TestResourceHandle:
    def test_minimal_handle(self):
        h = ResourceHandle.from_dict({"id": "res-1"})
        assert h.id == "res-1"
        assert h.mime_type is None
        assert h.size is None
        assert h.uri is None

    def test_full_handle(self):
        h = ResourceHandle.from_dict(
            {
                "id": "res-2",
                "mime_type": "image/png",
                "size": 4096,
                "uri": "saikuro://res/res-2",
            }
        )
        assert h.id == "res-2"
        assert h.mime_type == "image/png"
        assert h.size == 4096
        assert h.uri == "saikuro://res/res-2"

    def test_from_dict_requires_dict(self):
        with pytest.raises(ValueError):
            ResourceHandle.from_dict("not-a-dict")

    def test_from_dict_requires_string_id(self):
        with pytest.raises(ValueError):
            ResourceHandle.from_dict({"id": 42})

    def test_from_dict_requires_id_field(self):
        with pytest.raises(ValueError):
            ResourceHandle.from_dict({"mime_type": "text/plain"})

    def test_to_dict_minimal(self):
        h = ResourceHandle(id="r")
        d = h.to_dict()
        assert d == {"id": "r"}

    def test_to_dict_full(self):
        h = ResourceHandle(
            id="r", mime_type="text/plain", size=100, uri="saikuro://res/r"
        )
        d = h.to_dict()
        assert d["mime_type"] == "text/plain"
        assert d["size"] == 100
        assert d["uri"] == "saikuro://res/r"

    def test_round_trip(self):
        h = ResourceHandle(id="r", mime_type="application/json", size=256)
        restored = ResourceHandle.from_dict(h.to_dict())
        assert restored.id == h.id
        assert restored.mime_type == h.mime_type
        assert restored.size == h.size


# LogRecord


class TestLogRecord:
    def test_to_dict_required_fields(self):
        r = LogRecord(
            ts="2026-01-01T00:00:00Z", level=LogLevel.INFO, name="test", msg="hi"
        )
        d = r.to_dict()
        assert d["ts"] == "2026-01-01T00:00:00Z"
        assert d["level"] == "info"
        assert d["name"] == "test"
        assert d["msg"] == "hi"

    def test_to_dict_omits_empty_fields(self):
        r = LogRecord(ts="ts", level=LogLevel.DEBUG, name="n", msg="m")
        d = r.to_dict()
        assert "fields" not in d

    def test_to_dict_includes_fields_when_set(self):
        r = LogRecord(
            ts="ts", level=LogLevel.WARN, name="n", msg="m", fields={"k": "v"}
        )
        d = r.to_dict()
        assert d["fields"] == {"k": "v"}
