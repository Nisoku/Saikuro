use alloc::string::{ String, ToString };
use core::fmt;
use serde::{ Deserialize, Serialize };
use thiserror::Error;

use crate::io::{ IoError, IoErrorKind };
use crate::value::Value;

/// Maximum number of structured context entries an [`ErrorDetail`] can carry.
pub const ERROR_DETAIL_CAPACITY: usize = 16;

/// Fixed-capacity map of structured context entries on [`ErrorDetail`].
pub type DetailMap = heapless::FnvIndexMap<String, Value, ERROR_DETAIL_CAPACITY>;

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
    #[serde(default, skip_serializing_if = "DetailMap::is_empty")]
    pub details: DetailMap,
}

impl ErrorDetail {
    /// Construct a minimal error detail with a code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: DetailMap::new(),
        }
    }

    /// Add a detail entry and return `self` for chaining.
    /// Fails with [`SaikuroError::CapacityExceeded`] if the detail bag is
    /// already at [`ERROR_DETAIL_CAPACITY`] entries.
    pub fn with_detail(
        mut self,
        key: impl Into<String>,
        value: impl Into<Value>
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
pub enum SaikuroError {
    // Schema
    #[error("namespace not found: {0}")] NamespaceNotFound(String),

    #[error("function not found: {0}")] FunctionNotFound(String),

    #[error("invalid arguments for {target}: {reason}")] InvalidArguments {
        target: String,
        reason: String,
    },

    #[error(
        "incompatible protocol version: expected {expected}, got {received}"
    )] IncompatibleVersion {
        expected: u32,
        received: u32,
    },

    #[error("malformed envelope: {0}")] MalformedEnvelope(String),

    // Routing
    #[error("no provider registered for namespace: {0}")] NoProvider(String),

    #[error("provider unavailable for namespace: {0}")] ProviderUnavailable(String),

    #[error("batch routing conflict: {0}")] BatchRoutingConflict(String),

    // Capability
    #[error("capability denied: caller lacks '{required}' for '{target}'")] CapabilityDenied {
        target: String,
        required: String,
    },

    #[error("capability token invalid or expired")]
    CapabilityInvalid,

    // Transport
    #[error("transport connection lost: {0}")] ConnectionLost(String),

    #[error("message too large: {size} bytes exceeds limit {limit}")] MessageTooLarge {
        size: usize,
        limit: usize,
    },

    #[error("operation timed out after {millis}ms")] Timeout {
        millis: u64,
    },

    #[error("buffer overflow on stream/channel")]
    BufferOverflow,

    // Provider
    #[error("provider returned error: {0}")] ProviderError(String),

    #[error("provider panicked while handling invocation")]
    ProviderPanic,

    // Stream / channel
    #[error("stream already closed")]
    StreamClosed,

    #[error("channel closed by remote side")]
    ChannelClosed,

    #[error("out-of-order sequence: expected {expected}, got {received}")] OutOfOrder {
        expected: u64,
        received: u64,
    },

    //  Serialisation
    #[error("msgpack encode error: {0}")] MsgpackEncode(#[from] crate::msgpack::EncodeError),

    #[error("msgpack decode error: {0}")] MsgpackDecode(#[from] crate::msgpack::DecodeError),

    //  I/O
    #[error("I/O error: {0}")] Io(IoError),

    /// A fixed-capacity map reached its compile-time limit.
    #[error("capacity exceeded: {0}")]
    CapacityExceeded(String),

    //  Catch-all
    #[error("internal error: {0}")] Internal(String),
}

impl From<SaikuroError> for ErrorDetail {
    fn from(err: SaikuroError) -> Self {
        let code = match &err {
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
            SaikuroError::MsgpackEncode(_) | SaikuroError::MsgpackDecode(_) => ErrorCode::Internal,
            SaikuroError::Io(e) =>
                match e.kind {
                    IoErrorKind::TimedOut => ErrorCode::Timeout,
                    | IoErrorKind::ConnectionReset
                    | IoErrorKind::ConnectionAborted
                    | IoErrorKind::ConnectionRefused => ErrorCode::ConnectionLost,
                    _ => ErrorCode::Internal,
                }
            SaikuroError::CapacityExceeded(_) | SaikuroError::Internal(_) => ErrorCode::Internal,
        };

        ErrorDetail::new(code, err.to_string())
    }
}

/// Convert a host `std::io::Error` into the unified error type.
#[cfg(feature = "std")]
impl From<std::io::Error> for SaikuroError {
    fn from(err: std::io::Error) -> Self {
        SaikuroError::Io(err.into())
    }
}

/// Convenience alias for `Result<T, SaikuroError>`.
pub type Result<T> = core::result::Result<T, SaikuroError>;
