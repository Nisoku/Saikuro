//! Error types for the Saikuro system.
//!
//! Errors are modelled at two levels:
//!
//! 1. **[`SaikuroError`]** :  the Rust `std::error::Error`-implementing type
//!    used throughout the runtime for fallible operations.
//! 2. **[`ErrorDetail`]** :  the wire representation serialised into
//!    [`ResponseEnvelope`] when an invocation fails.  This is what remote
//!    adapters receive and surface to their callers.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

use crate::value::Value;

/// Machine-readable error codes transmitted on the wire.
///
/// Each variant maps to a distinct failure category so that adapters can
/// handle them appropriately without string parsing.
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

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Delegate to the derived Debug output which matches the serde names.
        write!(f, "{:?}", self)
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, Value>,
}

impl ErrorDetail {
    /// Construct a minimal error detail with a code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: BTreeMap::new(),
        }
    }

    /// Add a detail entry and return `self` for chaining.
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

impl std::fmt::Display for ErrorDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

/// The main Rust error type for all fallible Saikuro operations.
///
/// This is used internally by the runtime and its component crates.
/// When an error crosses the wire it is first converted to an [`ErrorDetail`]
/// via the [`From`] implementations below.
#[derive(Debug, Error)]
pub enum SaikuroError {
    //  Schema
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

    //  Routing
    #[error("no provider registered for namespace: {0}")]
    NoProvider(String),

    #[error("provider unavailable for namespace: {0}")]
    ProviderUnavailable(String),

    #[error("batch routing conflict: {0}")]
    BatchRoutingConflict(String),

    //  Capability
    #[error("capability denied: caller lacks '{required}' for '{target}'")]
    CapabilityDenied { target: String, required: String },

    #[error("capability token invalid or expired")]
    CapabilityInvalid,

    //  Transport
    #[error("transport connection lost: {0}")]
    ConnectionLost(String),

    #[error("message too large: {size} bytes exceeds limit {limit}")]
    MessageTooLarge { size: usize, limit: usize },

    #[error("operation timed out after {millis}ms")]
    Timeout { millis: u64 },

    #[error("buffer overflow on stream/channel")]
    BufferOverflow,

    //  Provider
    #[error("provider returned error: {0}")]
    ProviderError(String),

    #[error("provider panicked while handling invocation")]
    ProviderPanic,

    //  Stream / channel
    #[error("stream already closed")]
    StreamClosed,

    #[error("channel closed by remote side")]
    ChannelClosed,

    #[error("out-of-order sequence: expected {expected}, got {received}")]
    OutOfOrder { expected: u64, received: u64 },

    //  Serialisation
    #[error("msgpack encode error: {0}")]
    MsgpackEncode(#[from] rmp_serde::encode::Error),

    #[error("msgpack decode error: {0}")]
    MsgpackDecode(#[from] rmp_serde::decode::Error),

    //  I/O
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    //  Catch-all
    #[error("internal error: {0}")]
    Internal(String),
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
            SaikuroError::MsgpackEncode(_)
            | SaikuroError::MsgpackDecode(_)
            | SaikuroError::Io(_)
            | SaikuroError::Internal(_) => ErrorCode::Internal,
        };

        ErrorDetail::new(code, err.to_string())
    }
}

/// Convenience alias for `Result<T, SaikuroError>`.
pub type Result<T> = std::result::Result<T, SaikuroError>;
