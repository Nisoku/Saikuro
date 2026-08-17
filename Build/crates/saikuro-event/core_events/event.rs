use alloc::string::{String, ToString};
use core::fmt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::core_events::io::{IoError, IoErrorKind};
use crate::value::Value;

/// Maximum number of structured context entries an [`ErrorDetail`] or
/// [`LogRecord`] can carry.
pub const CONTEXT_CAPACITY: usize = 16;

/// Fixed-capacity map of structured context entries on [`ErrorDetail`] and
/// [`LogRecord`].
pub type ContextMap = heapless::FnvIndexMap<String, Value, CONTEXT_CAPACITY>;

/// All error codes transmitted on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ErrorCode {
    //  Schema errors
    /// The requested namespace is not registered.
    NamespaceNotFound,
    /// The requested function does not exist within its namespace.
    FunctionNotFound,
    /// One or more arguments failed type/shape validation.
    InvalidArguments,
    /// The schema version in the envelope is incompatible with this runtime.
    IncompatibleVersion,
    /// A required field was missing from an envelope.
    MalformedEnvelope,

    //  Routing errors
    /// No provider is registered for the target namespace.
    NoProvider,
    /// The provider for the target namespace is temporarily unavailable.
    ProviderUnavailable,
    /// A batch item's `target` resolved to a different namespace than allowed.
    BatchRoutingConflict,

    //  Capability errors
    /// The caller did not present the required capability token.
    CapabilityDenied,
    /// The capability token presented was invalid or expired.
    CapabilityInvalid,

    //  Transport errors
    /// The underlying transport connection was lost.
    ConnectionLost,
    /// A message exceeded the configured size limit.
    MessageTooLarge,
    /// The operation timed out waiting for a response.
    Timeout,
    /// The receive buffer overflowed due to backpressure violation.
    BufferOverflow,

    //  Provider errors
    /// The provider's handler returned an explicit error.
    ProviderError,
    /// The provider panicked while handling the invocation.
    ProviderPanic,

    //  Stream / channel errors
    /// A stream was already closed when an item was sent.
    StreamClosed,
    /// A channel was closed by the remote side.
    ChannelClosed,
    /// Out-of-order sequence number detected on an ordered stream.
    OutOfOrder,

    //  Storage errors
    /// A key does not exist in the namespace.
    KeyNotFound,
    /// The key already exists and the operation required it to be absent.
    KeyAlreadyExists,
    /// The namespace already exists and the operation required it to be absent.
    NamespaceAlreadyExists,
    /// The requested storage backend is not available on this target.
    BackendNotAvailable,
    /// The storage backend does not implement the requested operation.
    OperationNotSupported,
    /// The operation would exceed a configured storage or rate quota.
    QuotaExceeded,
    /// A value could not be serialized.
    Serialization,
    /// A value could not be deserialized.
    Deserialization,

    //  Additional transport errors
    /// The transport connection was refused by the remote endpoint.
    ConnectionRefused,
    /// A send over the transport failed.
    SendFailed,
    /// A receive over the transport failed.
    ReceiveFailed,
    /// The byte stream could not be framed into a message.
    FramingError,
    /// The transport is not supported on this target.
    TransportNotSupported,

    //  Additional routing errors
    /// A routing target was malformed (expected `namespace.function`).
    MalformedTarget,
    /// An entropy or DRBG operation failed.
    Entropy,
    /// The named stream does not exist.
    StreamNotFound,
    /// The named channel does not exist.
    ChannelNotFound,
    /// A send to a stream or channel failed.
    SendError,
    /// A batch item failed to dispatch.
    BatchItemFailed,

    //  Capacity errors
    /// A fixed-capacity collection reached its compile-time limit.
    CapacityExceeded,

    //  Catch-all
    /// An error category not covered by the above codes.
    Internal,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Delegate to the derived Debug output which matches the serde names.
        write!(f, "{self:?}")
    }
}

/// The wire-level error payload carried inside a failed [`ResponseEnvelope`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    /// Machine-readable code for programmatic handling.
    pub code: ErrorCode,

    /// Human-readable description, intended for log output and debugging.
    pub message: String,

    /// Optional structured context (stack traces, field paths, …).
    #[serde(default, skip_serializing_if = "ContextMap::is_empty")]
    pub details: ContextMap,
}

impl ErrorDetail {
    /// Construct a minimal error detail with a code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: ContextMap::new(),
        }
    }

    /// Add a context entry and return `self` for chaining.
    /// Fails with [`SaikuroError::CapacityExceeded`] if the context bag is
    /// already at [`CONTEXT_CAPACITY`] entries.
    pub fn with_context(
        mut self,
        key: impl Into<String>,
        value: impl Into<Value>,
    ) -> core::result::Result<Self, SaikuroError> {
        let key = key.into();
        self.details
            .insert(key.clone(), value.into())
            .map_err(|_| {
                SaikuroError::CapacityExceeded(format!("error detail bag full at key '{key}'"))
            })?;
        Ok(self)
    }
}

impl fmt::Display for ErrorDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

/// The main Rust error type for all fallible Saikuro operations.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum SaikuroError {
    // Schema
    #[error("namespace not found: {0}")]
    NamespaceNotFound(String),

    #[error("function not found: {0}")]
    FunctionNotFound(String),

    #[error("invalid arguments for {target}: {reason}")]
    InvalidArguments { target: String, reason: String },

    #[error("incompatible protocol version: expected {expected}, got {received}")]
    IncompatibleVersion { expected: u32, received: u32 },

    #[error("malformed envelope: {0}")]
    MalformedEnvelope(String),

    #[error("schema is frozen; updates are rejected: {0}")]
    FrozenSchema(String),

    #[error("schema capacity exceeded")]
    SchemaCapacity,

    #[error("batch envelope missing batch_items")]
    MissingBatch,

    #[error("batch envelope has no items")]
    EmptyBatch,

    #[error("visibility '{visibility}' denied for {target}")]
    VisibilityDenied { target: String, visibility: String },

    #[error("argument count mismatch: expected {expected}, got {received}")]
    ArgumentArity { expected: usize, received: usize },

    #[error("argument '{name}' (#{position}) expected {expected}, got {received}")]
    ArgumentType {
        name: String,
        position: usize,
        expected: String,
        received: String,
    },

    // Routing
    #[error("no provider registered for namespace: {0}")]
    NoProvider(String),

    #[error("provider unavailable for namespace: {0}")]
    ProviderUnavailable(String),

    #[error("batch routing conflict: {0}")]
    BatchRoutingConflict(String),

    // Capability
    #[error("capability denied: caller lacks '{required}' for '{target}'")]
    CapabilityDenied { target: String, required: String },

    #[error("capability token invalid or expired")]
    CapabilityInvalid,

    // Transport
    #[error("transport connection lost: {0}")]
    ConnectionLost(String),

    #[error("message too large: {size} bytes exceeds limit {limit}")]
    MessageTooLarge { size: usize, limit: usize },

    #[error("operation timed out after {millis}ms")]
    Timeout { millis: u64 },

    #[error("buffer overflow on stream/channel")]
    BufferOverflow,

    // Provider
    #[error("provider returned error: {0}")]
    ProviderError(String),

    #[error("provider panicked while handling invocation")]
    ProviderPanic,

    // Stream / channel
    #[error("stream already closed")]
    StreamClosed,

    #[error("channel closed by remote side")]
    ChannelClosed,

    #[error("out-of-order sequence: expected {expected}, got {received}")]
    OutOfOrder { expected: u64, received: u64 },

    //  Storage
    #[error("key not found: {0}")]
    KeyNotFound(String),

    #[error("key already exists: {0}")]
    KeyAlreadyExists(String),

    #[error("namespace already exists: {0}")]
    NamespaceAlreadyExists(String),

    #[error("storage backend not available: {0}")]
    BackendNotAvailable(String),

    #[error("operation not supported by backend: {0}")]
    OperationNotSupported(String),

    #[error("quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    //  Additional transport
    #[error("connection refused: {0}")]
    ConnectionRefused(String),

    #[error("transport send failed: {0}")]
    SendFailed(String),

    #[error("transport receive failed: {0}")]
    ReceiveFailed(String),

    #[error("framing error: {0}")]
    FramingError(String),

    #[error("transport not supported on this platform")]
    TransportNotSupported,

    //  Additional routing
    #[error("malformed target '{0}': must be 'namespace.function'")]
    MalformedTarget(String),

    #[error("stream not found: {0}")]
    StreamNotFound(String),

    #[error("channel not found: {0}")]
    ChannelNotFound(String),

    #[error("send error: {0}")]
    SendError(String),

    #[error("batch item {index} failed: {reason}")]
    BatchItemFailed { index: usize, reason: String },

    //  Entropy
    #[error("entropy error: {0}")]
    Entropy(String),

    //  Serialisation
    #[error("msgpack encode error: {0}")]
    MsgpackEncode(#[from] crate::core_events::codec::EncodeError),

    #[error("msgpack decode error: {0}")]
    MsgpackDecode(#[from] crate::core_events::codec::DecodeError),

    //  I/O
    #[error("I/O error: {0}")]
    Io(IoError),

    /// A fixed-capacity map reached its compile-time limit.
    #[error("capacity exceeded: {0}")]
    CapacityExceeded(String),

    //  Catch-all
    #[error("internal error: {0}")]
    Internal(String),
}

impl SaikuroError {
    /// The wire [`ErrorCode`] this error serialises as.
    pub fn error_code(&self) -> ErrorCode {
        match self {
            SaikuroError::NamespaceNotFound(_) => ErrorCode::NamespaceNotFound,
            SaikuroError::FunctionNotFound(_) => ErrorCode::FunctionNotFound,
            SaikuroError::InvalidArguments { .. } => ErrorCode::InvalidArguments,
            SaikuroError::IncompatibleVersion { .. } => ErrorCode::IncompatibleVersion,
            SaikuroError::MalformedEnvelope(_) => ErrorCode::MalformedEnvelope,
            SaikuroError::NoProvider(_) => ErrorCode::NoProvider,
            SaikuroError::ProviderUnavailable(_) => ErrorCode::ProviderUnavailable,
            SaikuroError::BatchRoutingConflict(_) => ErrorCode::BatchRoutingConflict,
            SaikuroError::CapabilityDenied { .. } => ErrorCode::CapabilityDenied,
            SaikuroError::CapabilityInvalid => ErrorCode::CapabilityInvalid,
            SaikuroError::ConnectionLost(_) => ErrorCode::ConnectionLost,
            SaikuroError::MessageTooLarge { .. } => ErrorCode::MessageTooLarge,
            SaikuroError::Timeout { .. } => ErrorCode::Timeout,
            SaikuroError::BufferOverflow => ErrorCode::BufferOverflow,
            SaikuroError::ProviderError(_) => ErrorCode::ProviderError,
            SaikuroError::ProviderPanic => ErrorCode::ProviderPanic,
            SaikuroError::StreamClosed => ErrorCode::StreamClosed,
            SaikuroError::ChannelClosed => ErrorCode::ChannelClosed,
            SaikuroError::OutOfOrder { .. } => ErrorCode::OutOfOrder,
            SaikuroError::KeyNotFound(_) => ErrorCode::KeyNotFound,
            SaikuroError::KeyAlreadyExists(_) => ErrorCode::KeyAlreadyExists,
            SaikuroError::NamespaceAlreadyExists(_) => ErrorCode::NamespaceAlreadyExists,
            SaikuroError::BackendNotAvailable(_) => ErrorCode::BackendNotAvailable,
            SaikuroError::OperationNotSupported(_) => ErrorCode::OperationNotSupported,
            SaikuroError::QuotaExceeded(_) => ErrorCode::QuotaExceeded,
            SaikuroError::Serialization(_) => ErrorCode::Serialization,
            SaikuroError::Deserialization(_) => ErrorCode::Deserialization,
            SaikuroError::ConnectionRefused(_) => ErrorCode::ConnectionRefused,
            SaikuroError::SendFailed(_) => ErrorCode::SendFailed,
            SaikuroError::ReceiveFailed(_) => ErrorCode::ReceiveFailed,
            SaikuroError::FramingError(_) => ErrorCode::FramingError,
            SaikuroError::TransportNotSupported => ErrorCode::TransportNotSupported,
            SaikuroError::MalformedTarget(_) => ErrorCode::MalformedTarget,
            SaikuroError::StreamNotFound(_) => ErrorCode::StreamNotFound,
            SaikuroError::ChannelNotFound(_) => ErrorCode::ChannelNotFound,
            SaikuroError::SendError(_) => ErrorCode::SendError,
            SaikuroError::BatchItemFailed { .. } => ErrorCode::BatchItemFailed,
            SaikuroError::Entropy(_) => ErrorCode::Entropy,
            SaikuroError::MsgpackEncode(_) | SaikuroError::MsgpackDecode(_) => ErrorCode::Internal,
            SaikuroError::Io(e) => match e.kind {
                IoErrorKind::TimedOut => ErrorCode::Timeout,
                IoErrorKind::ConnectionReset
                | IoErrorKind::ConnectionAborted
                | IoErrorKind::ConnectionRefused => ErrorCode::ConnectionLost,
                _ => ErrorCode::Internal,
            },
            SaikuroError::FrozenSchema(_) => ErrorCode::Internal,
            SaikuroError::SchemaCapacity => ErrorCode::CapacityExceeded,
            SaikuroError::MissingBatch => ErrorCode::MalformedEnvelope,
            SaikuroError::EmptyBatch => ErrorCode::MalformedEnvelope,
            SaikuroError::VisibilityDenied { .. } => ErrorCode::CapabilityDenied,
            SaikuroError::ArgumentArity { .. } => ErrorCode::InvalidArguments,
            SaikuroError::ArgumentType { .. } => ErrorCode::InvalidArguments,
            SaikuroError::CapacityExceeded(_) | SaikuroError::Internal(_) => ErrorCode::Internal,
        }
    }
}

impl SaikuroError {
    /// Construct a [`SaikuroError::KeyNotFound`].
    pub fn key_not_found(key: impl Into<String>) -> Self {
        SaikuroError::KeyNotFound(key.into())
    }

    /// Construct a [`SaikuroError::NamespaceNotFound`].
    pub fn namespace_not_found(namespace: impl Into<String>) -> Self {
        SaikuroError::NamespaceNotFound(namespace.into())
    }

    /// Construct a [`SaikuroError::KeyAlreadyExists`].
    pub fn key_already_exists(key: impl Into<String>) -> Self {
        SaikuroError::KeyAlreadyExists(key.into())
    }

    /// Construct a [`SaikuroError::NamespaceAlreadyExists`].
    pub fn namespace_already_exists(namespace: impl Into<String>) -> Self {
        SaikuroError::NamespaceAlreadyExists(namespace.into())
    }

    /// Construct a [`SaikuroError::Serialization`].
    pub fn serialization(msg: impl Into<String>) -> Self {
        SaikuroError::Serialization(msg.into())
    }

    /// Construct a [`SaikuroError::Deserialization`].
    pub fn deserialization(msg: impl Into<String>) -> Self {
        SaikuroError::Deserialization(msg.into())
    }

    /// Construct a [`SaikuroError::Internal`].
    pub fn internal(msg: impl Into<String>) -> Self {
        SaikuroError::Internal(msg.into())
    }

    /// Construct a [`SaikuroError::OperationNotSupported`].
    pub fn not_supported(msg: impl Into<String>) -> Self {
        SaikuroError::OperationNotSupported(msg.into())
    }

    /// Construct a [`SaikuroError::BackendNotAvailable`].
    pub fn backend_not_available(msg: impl Into<String>) -> Self {
        SaikuroError::BackendNotAvailable(msg.into())
    }

    /// Construct a [`SaikuroError::QuotaExceeded`].
    pub fn quota_exceeded(msg: impl Into<String>) -> Self {
        SaikuroError::QuotaExceeded(msg.into())
    }
}

impl From<SaikuroError> for ErrorDetail {
    fn from(err: SaikuroError) -> Self {
        ErrorDetail::new(err.error_code(), err.to_string())
    }
}

/// Convert a host `std::io::Error` into the unified error type.
#[cfg(feature = "std")]
impl From<std::io::Error> for SaikuroError {
    fn from(err: std::io::Error) -> Self {
        SaikuroError::Io(err.into())
    }
}

/// Convert a `getrandom` backend failure into the unified error type.
#[cfg(feature = "getrandom")]
impl From<getrandom::Error> for SaikuroError {
    fn from(err: getrandom::Error) -> Self {
        SaikuroError::Entropy(format!("entropy backend failed: {err}"))
    }
}

/// Convenience alias for `Result<T, SaikuroError>`.
pub type Result<T> = core::result::Result<T, SaikuroError>;
