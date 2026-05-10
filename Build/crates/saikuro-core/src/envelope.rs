//! Wire-level envelope types.
//!
//! Every message exchanged between a language adapter and the Saikuro runtime
//! is wrapped in an [`Envelope`] or [`ResponseEnvelope`].  Envelopes are
//! serialised to binary using MessagePack (via `rmp-serde`) before transit;
//! the types here are the canonical in-memory representation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    capability::CapabilityToken, invocation::InvocationId, value::Value, PROTOCOL_VERSION,
};

/// The type of an outgoing invocation.
///
/// This is the primary discriminator that tells the runtime :  and the
/// recipient adapter :  how to handle a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationType {
    /// Request/response: caller blocks until a single response arrives.
    Call,
    /// Fire-and-forget: no response is expected or sent.
    Cast,
    /// Server-to-client ordered sequence of messages on a single logical stream.
    Stream,
    /// Bidirectional ordered message stream with backpressure.
    Channel,
    /// Several independent calls bundled in one envelope to reduce round-trips.
    Batch,
    /// Reference to an opaque external resource (large payload, file handle, …).
    Resource,
    /// Structured log record forwarded from an adapter to the runtime log sink.
    ///
    /// Log envelopes are never routed to a provider.  The runtime extracts the
    /// [`LogRecord`](crate::log::LogRecord) from `args[0]` and passes it to the
    /// configured log sink.  No response envelope is sent.
    Log,
    /// Schema announcement sent by a provider immediately after connecting.
    ///
    /// The serialised [`Schema`](crate::schema::Schema) is packed as a
    /// MessagePack map in `args[0]`.  The runtime deserialises it and merges
    /// the namespaces into the live schema registry, then returns `ok_empty`.
    /// No provider is involved and no capability check is required.
    Announce,
}

impl std::fmt::Display for InvocationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Call => "call",
            Self::Cast => "cast",
            Self::Stream => "stream",
            Self::Channel => "channel",
            Self::Batch => "batch",
            Self::Resource => "resource",
            Self::Log => "log",
            Self::Announce => "announce",
        };
        f.write_str(s)
    }
}

/// Control frames sent within a stream or channel to signal lifecycle events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamControl {
    /// The sending side has no more items to send; the stream is half-closed.
    End,
    /// The receiver's buffer is full; the sender must pause until it receives
    /// a [`StreamControl::Resume`] frame.
    Pause,
    /// The receiver is ready for more data.
    Resume,
    /// An unrecoverable error occurred on the stream; both sides should close.
    Abort,
}

/// The outbound envelope carrying a single invocation from an adapter to
/// the runtime, or from the runtime to a provider adapter.
///
/// Fields follow the spec exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Protocol version :  must equal [`PROTOCOL_VERSION`].
    pub version: u32,

    /// What kind of invocation this is.
    #[serde(rename = "type")]
    pub invocation_type: InvocationType,

    /// Unique identifier for this invocation.
    ///
    /// Callers generate this; casts still carry an ID so they can be
    /// correlated in distributed traces even though no reply is sent.
    pub id: InvocationId,

    /// Fully-qualified target: `"<namespace>.<function>"`.
    pub target: String,

    /// Positional arguments.  Type checking happens in the runtime against
    /// the schema; adapters simply forward whatever the caller provided.
    #[serde(default)]
    pub args: Vec<Value>,

    /// Optional key/value metadata bag (trace IDs, deadlines, …).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, Value>,

    /// Capability token presented by the caller.  Required when the target
    /// function declares one or more `capabilities`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability: Option<CapabilityToken>,

    /// For [`InvocationType::Batch`]: the individual envelopes to execute.
    /// Must be `None` for every other invocation type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_items: Option<Vec<Envelope>>,

    /// For stream/channel messages that carry backpressure signals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_control: Option<StreamControl>,

    /// Sequence number within a stream or channel (per-direction, starts at 0).
    /// `None` for call/cast/batch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
}

impl Envelope {
    /// Construct the simplest possible call envelope.
    pub fn call(target: impl Into<String>, args: Vec<Value>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Call,
            id: InvocationId::new(),
            target: target.into(),
            args,
            meta: BTreeMap::new(),
            capability: None,
            batch_items: None,
            stream_control: None,
            seq: None,
        }
    }

    /// Construct a fire-and-forget cast envelope.
    pub fn cast(target: impl Into<String>, args: Vec<Value>) -> Self {
        Self {
            invocation_type: InvocationType::Cast,
            ..Self::call(target, args)
        }
    }

    /// Construct the initial envelope that opens a stream.
    pub fn stream_open(target: impl Into<String>, args: Vec<Value>) -> Self {
        Self {
            invocation_type: InvocationType::Stream,
            ..Self::call(target, args)
        }
    }

    /// Construct the initial envelope that opens a bidirectional channel.
    pub fn channel_open(target: impl Into<String>, args: Vec<Value>) -> Self {
        Self {
            invocation_type: InvocationType::Channel,
            ..Self::call(target, args)
        }
    }

    /// Construct a schema-announcement envelope.
    ///
    /// `schema_bytes` is the MessagePack-encoded [`Schema`](crate::schema::Schema)
    /// stored as a raw `Bytes` value in `args[0]`.
    pub fn announce(schema_value: Value) -> Self {
        Self {
            invocation_type: InvocationType::Announce,
            target: "$saikuro.announce".to_owned(),
            args: vec![schema_value],
            ..Self::call("$saikuro.announce", vec![])
        }
    }

    /// Construct a resource-access envelope.
    ///
    /// `target` is the provider function that manages the resource.
    /// `args` are provider-specific arguments that identify or parameterise
    /// the resource request (e.g. a resource ID, byte range, or query).
    pub fn resource(target: impl Into<String>, args: Vec<Value>) -> Self {
        Self {
            invocation_type: InvocationType::Resource,
            ..Self::call(target, args)
        }
    }

    /// Serialise this envelope to MessagePack bytes.
    pub fn to_msgpack(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec_named(self)
    }

    /// Deserialise an envelope from MessagePack bytes.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }

    /// Return the namespace portion of `target` (everything before the last `.`).
    pub fn namespace(&self) -> Option<&str> {
        split_target(&self.target).map(|(ns, _)| ns)
    }

    /// Return the function name portion of `target` (everything after the last `.`).
    pub fn function_name(&self) -> Option<&str> {
        split_target(&self.target).map(|(_, fn_name)| fn_name)
    }
}

/// Split a `"namespace.function"` target string into its two components.
///
/// Returns `None` when `target` contains no dot separator.
pub fn split_target(target: &str) -> Option<(&str, &str)> {
    let dot = target.rfind('.')?;
    Some((&target[..dot], &target[dot + 1..]))
}

/// The envelope carrying a response back to a caller.
///
/// Fields follow the spec exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    /// The ID from the originating [`Envelope`].
    pub id: InvocationId,

    /// `true` if the invocation succeeded; `false` otherwise.
    pub ok: bool,

    /// Successful return value.  `None` when `ok` is `false` or the function
    /// returns nothing meaningful (e.g. casts, pure side-effects).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,

    /// Error detail present when `ok` is `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<crate::error::ErrorDetail>,

    /// For streaming responses: the sequence number of this item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,

    /// For streaming responses: backpressure / lifecycle signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_control: Option<StreamControl>,
}

impl ResponseEnvelope {
    /// Construct a successful response carrying a result value.
    pub fn ok(id: InvocationId, result: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
            seq: None,
            stream_control: None,
        }
    }

    /// Construct a successful response with no meaningful return value.
    pub fn ok_empty(id: InvocationId) -> Self {
        Self {
            id,
            ok: true,
            result: None,
            error: None,
            seq: None,
            stream_control: None,
        }
    }

    /// Construct an error response.
    pub fn err(id: InvocationId, detail: crate::error::ErrorDetail) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(detail),
            seq: None,
            stream_control: None,
        }
    }

    /// Construct a streaming item response.
    pub fn stream_item(id: InvocationId, seq: u64, value: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(value),
            error: None,
            seq: Some(seq),
            stream_control: None,
        }
    }

    /// Construct the end-of-stream sentinel.
    pub fn stream_end(id: InvocationId, seq: u64) -> Self {
        Self {
            id,
            ok: true,
            result: None,
            error: None,
            seq: Some(seq),
            stream_control: Some(StreamControl::End),
        }
    }

    /// Serialise this response to MessagePack bytes.
    pub fn to_msgpack(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        rmp_serde::to_vec_named(self)
    }

    /// Deserialise a response from MessagePack bytes.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}
