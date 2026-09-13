"""
Wire envelope types for the Saikuro protocol.

Mirrors the Rust saikuro_core::envelope module.  All values are encoded as
MessagePack on the wire; Python dicts are used as the in-process
representation.
"""

from __future__ import annotations

import enum
import uuid
from dataclasses import dataclass, field
from typing import Any


class InvocationType(enum.Enum):
    """Maps to the Rust InvocationType enum."""

    CALL = "call"
    CAST = "cast"
    STREAM = "stream"
    CHANNEL = "channel"
    BATCH = "batch"
    RESOURCE = "resource"
    LOG = "log"
    ANNOUNCE = "announce"


class StreamControl(enum.Enum):
    END = "end"
    PAUSE = "pause"
    RESUME = "resume"
    ABORT = "abort"


class LogLevel(enum.Enum):
    """Severity levels for structured log records forwarded over the transport."""

    TRACE = "trace"
    DEBUG = "debug"
    INFO = "info"
    WARN = "warn"
    ERROR = "error"


@dataclass
class LogRecord:
    """
    A structured log record forwarded from an adapter to the runtime log sink.

    Matches the Rust ``saikuro_core::log::LogRecord`` type.
    """

    ts: str  # ISO-8601 timestamp
    level: LogLevel
    name: str  # logger name / origin
    msg: str
    fields: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        """Serialise to a plain dict for embedding in an Envelope ``args`` list."""
        d: dict[str, Any] = {
            "ts": self.ts,
            "level": self.level.value,
            "name": self.name,
            "msg": self.msg,
        }
        if self.fields:
            d["fields"] = self.fields
        return d


# Resource handle


@dataclass
class ResourceHandle:
    """
    An opaque reference to large or external data.

    Returned as the ``result`` of a ``resource``-type invocation. All fields
    except ``id`` are optional.
    """

    id: str
    mime_type: str | None = None
    size: int | None = None
    uri: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Serialise to a plain dict for embedding in a response ``result``."""
        d: dict[str, Any] = {"id": self.id}
        if self.mime_type is not None:
            d["mime_type"] = self.mime_type
        if self.size is not None:
            d["size"] = self.size
        if self.uri is not None:
            d["uri"] = self.uri
        return d

    @classmethod
    def from_dict(cls, d: dict[str, Any]) -> ResourceHandle:
        """Deserialise from a plain dict (as decoded from MessagePack).

        Raises :class:`TypeError` if ``d`` is not a mapping or the ``id``
        field is not a string.
        """
        if not isinstance(d, dict):
            raise TypeError(
                f"ResourceHandle.from_dict: expected dict, got {type(d).__name__!r}"
            )
        id_val = d.get("id")
        if not isinstance(id_val, str):
            raise TypeError(
                "ResourceHandle.from_dict: missing or non-string 'id' field"
            )
        return cls(
            id=id_val,
            mime_type=d.get("mime_type") or None,
            size=d.get("size"),
            uri=d.get("uri") or None,
        )

    def __str__(self) -> str:
        parts = [f"Resource({self.id})"]
        if self.mime_type:
            parts.append(f"[{self.mime_type}]")
        if self.size is not None:
            parts.append(f"{self.size}B")
        return " ".join(parts)


@dataclass
class Envelope:
    """Outbound invocation envelope."""

    version: int
    invocation_type: InvocationType
    id: str
    target: str
    args: list[Any] = field(default_factory=list)
    meta: dict[str, Any] = field(default_factory=dict)
    capability: str | None = None
    batch_items: list[Envelope] | None = None
    stream_control: StreamControl | None = None
    seq: int | None = None

    @classmethod
    def make_call(
        cls, target: str, args: list[Any], capability: str | None = None
    ) -> Envelope:
        return cls(
            version=1,
            invocation_type=InvocationType.CALL,
            id=str(uuid.uuid4()),
            target=target,
            args=args,
            capability=capability,
        )

    @classmethod
    def make_cast(
        cls, target: str, args: list[Any], capability: str | None = None
    ) -> Envelope:
        env = cls.make_call(target, args, capability)
        env.invocation_type = InvocationType.CAST
        return env

    @classmethod
    def make_stream_open(cls, target: str, args: list[Any]) -> Envelope:
        env = cls.make_call(target, args)
        env.invocation_type = InvocationType.STREAM
        return env

    @classmethod
    def make_channel_open(cls, target: str, args: list[Any]) -> Envelope:
        env = cls.make_call(target, args)
        env.invocation_type = InvocationType.CHANNEL
        return env

    @classmethod
    def make_announce(cls, schema_dict: dict[str, Any]) -> Envelope:
        """Construct a schema-announcement envelope.

        ``schema_dict`` is the plain-dict representation of a Schema produced by
        :py:meth:`saikuro.schema.SchemaBuilder.build`. It is embedded verbatim
        as the first argument; the runtime deserialises it and merges it into the
        live registry.
        """
        return cls(
            version=1,
            invocation_type=InvocationType.ANNOUNCE,
            id=str(uuid.uuid4()),
            target="$saikuro.announce",
            args=[schema_dict],
        )

    @classmethod
    def make_batch(cls, items: list[Envelope]) -> Envelope:
        """Construct a batch invocation envelope.

        ``items`` are the individual call envelopes to execute together. The
        runtime dispatches each item independently and returns an ordered list
        of results in the response (``None`` for failed items).
        """
        batch_id = str(uuid.uuid4())
        return cls(
            version=1,
            invocation_type=InvocationType.BATCH,
            id=batch_id,
            target="",
            args=[],
            batch_items=list(items),
        )

    @classmethod
    def make_resource(
        cls, target: str, args: list[Any], capability: str | None = None
    ) -> Envelope:
        """Construct a resource-access envelope.

        ``target`` identifies the provider function that manages the resource
        (e.g. ``"files.open"``).  ``args`` are provider-specific parameters
        that identify or parameterise the resource.  The provider returns a
        :class:`ResourceHandle` in the response ``result``.
        """
        env = cls.make_call(target, args, capability)
        env.invocation_type = InvocationType.RESOURCE
        return env

    def to_msgpack_dict(self) -> dict[str, Any]:
        """Serialise to the dict that will be MessagePack-encoded."""
        d: dict[str, Any] = {
            "version": self.version,
            "type": self.invocation_type.value,
            "id": self.id,
            "target": self.target,
            "args": self.args,
        }
        if self.meta:
            d["meta"] = self.meta
        if self.capability is not None:
            d["capability"] = self.capability
        if self.batch_items is not None:
            d["batch_items"] = [item.to_msgpack_dict() for item in self.batch_items]
        if self.stream_control is not None:
            d["stream_control"] = self.stream_control.value
        if self.seq is not None:
            d["seq"] = self.seq
        return d

    @classmethod
    def from_msgpack_dict(cls, d: dict[str, Any]) -> Envelope:
        batch_items = None
        if "batch_items" in d:
            batch_items = [cls.from_msgpack_dict(item) for item in d["batch_items"]]
        sc = d.get("stream_control")
        return cls(
            version=d["version"],
            invocation_type=InvocationType(d["type"]),
            id=d["id"],
            target=d["target"],
            args=d.get("args", []),
            meta=d.get("meta", {}),
            capability=d.get("capability"),
            batch_items=batch_items,
            stream_control=StreamControl(sc) if sc else None,
            seq=d.get("seq"),
        )


@dataclass
class ResponseEnvelope:
    """Inbound response envelope."""

    id: str
    ok: bool
    result: Any = None
    error: dict[str, Any] | None = None
    seq: int | None = None
    stream_control: StreamControl | None = None

    @classmethod
    def from_msgpack_dict(cls, d: dict[str, Any]) -> ResponseEnvelope:
        sc = d.get("stream_control")
        return cls(
            id=d["id"],
            ok=d["ok"],
            result=d.get("result"),
            error=d.get("error"),
            seq=d.get("seq"),
            stream_control=StreamControl(sc) if sc else None,
        )

    @property
    def is_stream_end(self) -> bool:
        return self.stream_control in (StreamControl.END, StreamControl.ABORT)
