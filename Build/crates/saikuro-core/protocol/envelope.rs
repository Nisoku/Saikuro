use alloc::{string::String, vec::Vec};
use serde::{
    ser::{SerializeMap, Serializer},
    Deserialize, Serialize,
};

use crate::{
    capability::CapabilityToken, invocation::InvocationId, value::Value, PROTOCOL_VERSION,
};

/// Maximum number of key/value metadata entries an [`Envelope`] can carry.
pub const ENVELOPE_META_CAPACITY: usize = 16;

/// Fixed-capacity map of metadata entries on an [`Envelope`].
pub type MetaMap = heapless::FnvIndexMap<String, Value, ENVELOPE_META_CAPACITY>;

/// Serialize the metadata map with keys sorted, so equivalent metadata always
/// produces identical bytes regardless of the caller's insertion order.
fn serialize_meta<S>(meta: &MetaMap, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut pairs: Vec<(&str, &Value)> = meta.iter().map(|(k, v)| (k.as_str(), v)).collect();
    pairs.sort_unstable_by(|a, b| a.0.cmp(b.0));
    let mut map = serializer.serialize_map(Some(pairs.len()))?;
    for (key, value) in pairs {
        map.serialize_entry(key, value)?;
    }
    map.end()
}

/// The type of an outgoing invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
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
    Log,
    /// Schema announcement sent by a provider immediately after connecting.
    Announce,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    /// Protocol version: must equal [`PROTOCOL_VERSION`].
    pub version: u32,

    /// What kind of invocation this is.
    #[serde(rename = "type")]
    pub invocation_type: InvocationType,

    /// Unique identifier for this invocation.
    pub id: InvocationId,

    /// Fully-qualified target: `"<namespace>.<function>"`.
    pub target: String,

    /// Positional arguments.
    #[serde(default)]
    pub args: Vec<Value>,

    /// Optional key/value metadata bag (trace IDs, deadlines, …).
    #[serde(
        default,
        skip_serializing_if = "MetaMap::is_empty",
        serialize_with = "serialize_meta"
    )]
    pub meta: MetaMap,

    /// Capability token presented by the caller. Required when the target
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

// Shared MessagePack serialization for wire types.
macro_rules! impl_msgpack {
    ($ty:ty) => {
        impl $ty {
            /// Serialise this envelope to MessagePack bytes.
            pub fn to_msgpack(&self) -> Result<Vec<u8>, crate::msgpack::EncodeError> {
                crate::msgpack::to_vec(self)
            }

            /// Deserialise from MessagePack bytes.
            pub fn from_msgpack(bytes: &[u8]) -> Result<Self, crate::msgpack::DecodeError> {
                crate::msgpack::from_slice(bytes)
            }
        }
    };
}

impl_msgpack!(Envelope);
impl_msgpack!(ResponseEnvelope);

impl Envelope {
    /// Construct the simplest possible call envelope.
    pub fn call(
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self, saikuro_random::Error> {
        Ok(Self {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Call,
            id: InvocationId::new()?,
            target: target.into(),
            args,
            meta: MetaMap::new(),
            capability: None,
            batch_items: None,
            stream_control: None,
            seq: None,
        })
    }

    /// Construct a fire-and-forget cast envelope.
    pub fn cast(
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self, saikuro_random::Error> {
        let mut envelope = Self::call(target, args)?;
        envelope.invocation_type = InvocationType::Cast;
        Ok(envelope)
    }

    /// Construct the initial envelope that opens a stream.
    pub fn stream_open(
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self, saikuro_random::Error> {
        let mut envelope = Self::call(target, args)?;
        envelope.invocation_type = InvocationType::Stream;
        Ok(envelope)
    }

    /// Construct the initial envelope that opens a bidirectional channel.
    pub fn channel_open(
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self, saikuro_random::Error> {
        let mut envelope = Self::call(target, args)?;
        envelope.invocation_type = InvocationType::Channel;
        Ok(envelope)
    }

    /// Construct a schema-announcement envelope.
    pub fn announce(schema_value: Value) -> Result<Self, saikuro_random::Error> {
        let mut envelope = Self::call("$saikuro.announce", vec![schema_value])?;
        envelope.invocation_type = InvocationType::Announce;
        Ok(envelope)
    }

    /// Construct a resource-access envelope.
    pub fn resource(
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self, saikuro_random::Error> {
        let mut envelope = Self::call(target, args)?;
        envelope.invocation_type = InvocationType::Resource;
        Ok(envelope)
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
pub fn split_target(target: &str) -> Option<(&str, &str)> {
    let dot = target.rfind('.')?;
    if dot == 0 || dot == target.len() - 1 {
        return None;
    }
    Some((&target[..dot], &target[dot + 1..]))
}

/// The envelope carrying a response back to a caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    /// The ID from the originating [`Envelope`].
    pub id: InvocationId,

    /// `true` if the invocation succeeded; `false` otherwise.
    pub ok: bool,

    /// Successful return value. `None` when `ok` is `false` or the function
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
}
