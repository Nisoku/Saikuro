use alloc::{boxed::Box, string::String, vec::Vec};
use messagepack_serde::messagepack_core::{
    decode::{DecodeBorrowed, NbyteReader},
    io::{IoRead, SliceReader},
    Format,
};
use serde::{
    ser::{SerializeMap, Serializer},
    Deserialize, Serialize,
};

use crate::{capability::CapabilityToken, invocation::InvocationId, PROTOCOL_VERSION};
use saikuro_event::Value;

/// Maximum number of key/value metadata entries an [`Envelope`] can carry.
pub const ENVELOPE_META_CAPACITY: usize = 16;

/// Fixed-capacity map of metadata entries on an [`Envelope`].
pub type MetaMap = heapless::FnvIndexMap<String, Value, ENVELOPE_META_CAPACITY>;

/// Serialize the metadata map with keys sorted, so equivalent metadata always
/// produces identical bytes regardless of the caller's insertion order.
fn serialize_meta<S>(meta: &Option<Box<MetaMap>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let Some(map) = meta.as_deref() else {
        return serializer.serialize_map(Some(0))?.end();
    };
    let mut pairs: Vec<(&str, &Value)> = map.iter().map(|(k, v)| (k.as_str(), v)).collect();
    pairs.sort_unstable_by(|a, b| a.0.cmp(b.0));
    let mut map = serializer.serialize_map(Some(pairs.len()))?;
    for (key, value) in pairs {
        map.serialize_entry(key, value)?;
    }
    map.end()
}

/// `true` when the metadata bag is absent or empty, so the field is omitted
/// from the wire format.
fn meta_is_empty(meta: &Option<Box<MetaMap>>) -> bool {
    meta.as_deref().is_none_or(MetaMap::is_empty)
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
///
/// `A` is the positional-argument type. Callers construct and decode with the
/// default `Value`; the runtime decodes announce frames as [`Envelope<Schema>`]
/// so the schema in `args[0]` parses typed instead of through serde's untagged
/// `Value` path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<A = Value> {
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
    #[serde(default = "default_args")]
    pub args: Vec<A>,

    /// Optional key/value metadata bag (trace IDs, deadlines, …).
    #[serde(
        default,
        skip_serializing_if = "meta_is_empty",
        serialize_with = "serialize_meta"
    )]
    pub meta: Option<Box<MetaMap>>,

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

/// A default `args` value for a missing field, without requiring `A: Default`.
fn default_args<A>() -> Vec<A> {
    Vec::new()
}

impl<A> Envelope<A> {
    /// Borrow the metadata bag, if present. Envelopes constructed without
    /// metadata carry no bag and return `None`.
    pub fn meta(&self) -> Option<&MetaMap> {
        self.meta.as_deref()
    }

    /// Mutably borrow the metadata bag, allocating the backing map on first
    /// access.
    pub fn meta_mut(&mut self) -> &mut MetaMap {
        self.meta.get_or_insert_with(|| Box::new(MetaMap::new()))
    }
}

// Shared MessagePack serialization for wire types.
macro_rules! impl_msgpack {
    ($ty:ty) => {
        impl $ty {
            /// Serialise this envelope to MessagePack bytes.
            pub fn to_msgpack(&self) -> Result<Vec<u8>, saikuro_event::EncodeError> {
                crate::msgpack::to_vec(self)
            }

            /// Deserialise from MessagePack bytes.
            pub fn from_msgpack(bytes: &[u8]) -> Result<Self, saikuro_event::DecodeError> {
                crate::msgpack::from_slice(bytes)
            }
        }
    };
}

impl<A: Serialize> Envelope<A> {
    /// Serialise this envelope to MessagePack bytes.
    pub fn to_msgpack(&self) -> Result<Vec<u8>, saikuro_event::EncodeError> {
        crate::msgpack::to_vec(self)
    }
}

impl<'de, A: Deserialize<'de>> Envelope<A> {
    /// Deserialise a frame whose positional arguments decode as `A`
    /// (e.g. [`crate::schema::Schema`] for announce frames).
    pub fn from_msgpack_typed(bytes: &'de [u8]) -> Result<Self, saikuro_event::DecodeError> {
        crate::msgpack::from_slice(bytes)
    }
}

impl_msgpack!(ResponseEnvelope);

impl Envelope<Value> {
    /// Deserialise from MessagePack bytes into a `Value`-argued envelope.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, saikuro_event::DecodeError> {
        crate::msgpack::from_slice(bytes)
    }
    /// Construct the simplest possible call envelope.
    pub fn call(
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self, saikuro_event::SaikuroError> {
        Ok(Self {
            version: PROTOCOL_VERSION,
            invocation_type: InvocationType::Call,
            id: InvocationId::new()?,
            target: target.into(),
            args,
            meta: None,
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
    ) -> Result<Self, saikuro_event::SaikuroError> {
        let mut envelope = Self::call(target, args)?;
        envelope.invocation_type = InvocationType::Cast;
        Ok(envelope)
    }

    /// Construct the initial envelope that opens a stream.
    pub fn stream_open(
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self, saikuro_event::SaikuroError> {
        let mut envelope = Self::call(target, args)?;
        envelope.invocation_type = InvocationType::Stream;
        Ok(envelope)
    }

    /// Construct the initial envelope that opens a bidirectional channel.
    pub fn channel_open(
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self, saikuro_event::SaikuroError> {
        let mut envelope = Self::call(target, args)?;
        envelope.invocation_type = InvocationType::Channel;
        Ok(envelope)
    }

    /// Construct a schema-announcement envelope.
    pub fn announce(schema_value: Value) -> Result<Self, saikuro_event::SaikuroError> {
        let mut envelope = Self::call("$saikuro.announce", vec![schema_value])?;
        envelope.invocation_type = InvocationType::Announce;
        Ok(envelope)
    }

    /// Construct a resource-access envelope.
    pub fn resource(
        target: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self, saikuro_event::SaikuroError> {
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

// Frame discrimination

/// Classify a serialized envelope frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// The frame is a [`ResponseEnvelope`].
    Response,
    /// The frame is an [`InvocationType::Announce`] [`Envelope`].
    Announce,
    /// The frame is any other non-response [`Envelope`].
    NonResponse,
}

/// Classify a frame by scanning its top-level MessagePack map keys.
///
/// Unparseable or non-map frames classify as [`FrameKind::NonResponse`] so the
/// regular decode path reports the malformed-envelope error.
pub fn classify_frame(frame: &[u8]) -> FrameKind {
    let mut reader = SliceReader::new(frame);
    let format = match <Format as DecodeBorrowed>::decode_borrowed(&mut reader) {
        Ok(f) => f,
        Err(_) => return FrameKind::NonResponse,
    };
    let entries = match format {
        Format::FixMap(n) => n as usize,
        Format::Map16 => match NbyteReader::<2>::read(&mut reader) {
            Ok(n) => n,
            Err(_) => return FrameKind::NonResponse,
        },
        Format::Map32 => match NbyteReader::<4>::read(&mut reader) {
            Ok(n) => n,
            Err(_) => return FrameKind::NonResponse,
        },
        _ => return FrameKind::NonResponse,
    };

    for _ in 0..entries {
        let key = match scan_top_key(&mut reader) {
            Some(k) => k,
            None => return FrameKind::NonResponse,
        };
        match key {
            TopKey::Stop(kind) => return kind,
            TopKey::Type => {
                return if type_is_announce(&mut reader) {
                    FrameKind::Announce
                } else {
                    FrameKind::NonResponse
                };
            }
            TopKey::Continue => {
                if !skip_value(&mut reader) {
                    return FrameKind::NonResponse;
                }
            }
            TopKey::Consumed => {}
        }
    }

    FrameKind::NonResponse
}

/// Outcome of reading one top-level map key.
enum TopKey {
    /// The scan can stop: the key identifies the frame kind.
    Stop(FrameKind),
    /// The key is `type`; the caller reads its value next.
    Type,
    /// Unknown string key; the caller skips its value.
    Continue,
    /// A non-string key whose key and value were both skipped already.
    Consumed,
}

/// Classify a top-level key's name.
fn top_key(kind: &[u8]) -> TopKey {
    match kind {
        b"ok" => TopKey::Stop(FrameKind::Response),
        b"type" => TopKey::Type,
        _ => TopKey::Continue,
    }
}

/// Read and classify the next top-level map key. Returns `None` when the
/// frame is malformed, so the caller reports a malformed-envelope error.
fn scan_top_key(reader: &mut SliceReader<'_>) -> Option<TopKey> {
    let key_format = match <Format as DecodeBorrowed>::decode_borrowed(reader) {
        Ok(f) => f,
        Err(_) => return None,
    };
    match key_format {
        Format::FixStr(n) => read_fixed_str_key(reader, n as usize),
        Format::Str8 => scan_top_key_n(reader, 1),
        Format::Str16 => scan_top_key_n(reader, 2),
        Format::Str32 => scan_top_key_n(reader, 4),
        // A non-string top-level key is not required by the protocol; fully
        // consume the key and value so subsequent entries stay aligned.
        _ => {
            if !skip_value_with_format(reader, key_format) {
                return None;
            }
            if !skip_value(reader) {
                return None;
            }
            Some(TopKey::Consumed)
        }
    }
}

/// Read a fixed-length string top-level key and classify it.
fn read_fixed_str_key(reader: &mut SliceReader<'_>, len: usize) -> Option<TopKey> {
    let key = reader.read_slice(len).ok()?;
    Some(top_key(key.as_bytes()))
}

/// Read a length-prefixed string top-level key and classify it.
fn scan_top_key_n(reader: &mut SliceReader<'_>, len_bytes: usize) -> Option<TopKey> {
    let len = read_len(reader, len_bytes).ok()?;
    read_fixed_str_key(reader, len)
}

/// Read the value of the `type` top-level key and report whether it reads
/// `"announce"`.
fn type_is_announce(reader: &mut SliceReader<'_>) -> bool {
    let format = match <Format as DecodeBorrowed>::decode_borrowed(reader) {
        Ok(f) => f,
        Err(_) => return false,
    };
    match format {
        Format::FixStr(n) => read_fixed_type(reader, n as usize),
        Format::Str8 => type_str_is_announce(reader, 1),
        Format::Str16 => type_str_is_announce(reader, 2),
        Format::Str32 => type_str_is_announce(reader, 4),
        _ => {
            skip_value_with_format(reader, format);
            false
        }
    }
}

/// Read a fixed-length `type` value and report whether it reads `"announce"`.
fn read_fixed_type(reader: &mut SliceReader<'_>, len: usize) -> bool {
    match reader.read_slice(len) {
        Ok(v) => matches!(v.as_bytes(), b"announce"),
        Err(_) => false,
    }
}

/// Read a length-prefixed `type` value and report whether it reads
/// `"announce"`.
fn type_str_is_announce(reader: &mut SliceReader<'_>, len_bytes: usize) -> bool {
    let len = match read_len(reader, len_bytes) {
        Ok(len) => len,
        Err(_) => return false,
    };
    read_fixed_type(reader, len)
}

/// Skip one MessagePack value, reading its format marker first.
fn skip_value(reader: &mut SliceReader<'_>) -> bool {
    let format = match <Format as DecodeBorrowed>::decode_borrowed(reader) {
        Ok(f) => f,
        Err(_) => return false,
    };
    skip_value_with_format(reader, format)
}

/// Skip one MessagePack value whose format marker is already known.
fn skip_value_with_format(reader: &mut SliceReader<'_>, format: Format) -> bool {
    match format {
        Format::PositiveFixInt(_)
        | Format::NegativeFixInt(_)
        | Format::Uint8
        | Format::Uint16
        | Format::Uint32
        | Format::Uint64
        | Format::Int8
        | Format::Int16
        | Format::Int32
        | Format::Int64
        | Format::Nil
        | Format::NeverUsed
        | Format::False
        | Format::True => true,

        Format::FixMap(n) => skip_map_entries(reader, n as usize),
        Format::Map16 => skip_map_n(reader, 2),
        Format::Map32 => skip_map_n(reader, 4),
        Format::FixArray(n) => skip_values(reader, n as usize),
        Format::Array16 => skip_array_n(reader, 2),
        Format::Array32 => skip_array_n(reader, 4),

        Format::FixStr(n) => reader.read_slice(n as usize).is_ok(),
        Format::Str8 => skip_sized(reader, 1),
        Format::Str16 => skip_sized(reader, 2),
        Format::Str32 => skip_sized(reader, 4),

        Format::Bin8 => skip_sized(reader, 1),
        Format::Bin16 => skip_sized(reader, 2),
        Format::Bin32 => skip_sized(reader, 4),

        Format::Float32 => reader.read_slice(4).is_ok(),
        Format::Float64 => reader.read_slice(8).is_ok(),

        Format::FixExt1 => skip_ext(reader, 1),
        Format::FixExt2 => skip_ext(reader, 2),
        Format::FixExt4 => skip_ext(reader, 4),
        Format::FixExt8 => skip_ext(reader, 8),
        Format::FixExt16 => skip_ext(reader, 16),
        Format::Ext8 => skip_ext_n(reader, 1),
        Format::Ext16 => skip_ext_n(reader, 2),
        Format::Ext32 => skip_ext_n(reader, 4),
    }
}

/// Skip `count` consecutive MessagePack values.
fn skip_values(reader: &mut SliceReader<'_>, count: usize) -> bool {
    for _ in 0..count {
        if !skip_value(reader) {
            return false;
        }
    }
    true
}

/// Skip `count` map entries (each entry is a key/value pair).
fn skip_map_entries(reader: &mut SliceReader<'_>, count: usize) -> bool {
    for _ in 0..count {
        if !skip_value(reader) || !skip_value(reader) {
            return false;
        }
    }
    true
}

/// Skip a map whose entry count is a `len_bytes`-wide unsigned integer.
fn skip_map_n(reader: &mut SliceReader<'_>, len_bytes: usize) -> bool {
    match read_len(reader, len_bytes) {
        Ok(n) => skip_map_entries(reader, n),
        Err(_) => false,
    }
}

/// Skip an array whose length is a `len_bytes`-wide unsigned integer.
fn skip_array_n(reader: &mut SliceReader<'_>, len_bytes: usize) -> bool {
    match read_len(reader, len_bytes) {
        Ok(n) => skip_values(reader, n),
        Err(_) => false,
    }
}

/// Skip a bin/str whose length is a `len_bytes`-wide unsigned integer.
fn skip_sized(reader: &mut SliceReader<'_>, len_bytes: usize) -> bool {
    match read_len(reader, len_bytes) {
        Ok(len) => reader.read_slice(len).is_ok(),
        Err(_) => false,
    }
}

/// Skip an extension of `len` data bytes plus its one type byte.
fn skip_ext(reader: &mut SliceReader<'_>, len: usize) -> bool {
    reader.read_slice(len).is_ok() && reader.read_slice(1).is_ok()
}

/// Skip an extension whose data length is a `len_bytes`-wide unsigned integer.
fn skip_ext_n(reader: &mut SliceReader<'_>, len_bytes: usize) -> bool {
    match read_len(reader, len_bytes) {
        Ok(len) => skip_ext(reader, len),
        Err(_) => false,
    }
}

/// Read a big-endian `len_bytes`-wide unsigned integer used as a length field.
fn read_len(reader: &mut SliceReader<'_>, len_bytes: usize) -> Result<usize, ()> {
    match len_bytes {
        1 => NbyteReader::<1>::read(reader).map_err(|_| ()),
        2 => NbyteReader::<2>::read(reader).map_err(|_| ()),
        4 => NbyteReader::<4>::read(reader).map_err(|_| ()),
        _ => Err(()),
    }
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
    pub error: Option<saikuro_event::ErrorDetail>,

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
    pub fn err(id: InvocationId, detail: saikuro_event::ErrorDetail) -> Self {
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
