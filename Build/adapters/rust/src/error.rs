//! Error types for the Saikuro Rust adapter.

use thiserror::Error;

/// The result type used throughout this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// All errors that can be produced by the Saikuro adapter.
#[derive(Debug, Error)]
pub enum Error {
    /// The remote function call returned an error response.
    #[error("saikuro error [{code}]: {message}")]
    Remote {
        /// Machine-readable error code (e.g. `"CapabilityDenied"`).
        code: String,
        /// Human-readable description.
        message: String,
        /// Optional structured detail map.
        details: Option<serde_json::Value>,
    },

    /// The transport could not be connected or the connection was lost.
    #[error("transport error: {0}")]
    Transport(String),

    /// A timeout elapsed before a response arrived.
    #[error("call to '{target}' timed out after {ms}ms")]
    Timeout { target: String, ms: u64 },

    /// A response arrived for an unknown invocation ID.
    #[error("unexpected response for id '{0}'")]
    UnexpectedResponse(String),

    /// Serialization or deserialization failed.
    #[error("codec error: {0}")]
    Codec(String),

    /// The client or provider is not in the correct state for this operation.
    #[error("invalid state: {0}")]
    InvalidState(String),
}

impl Error {
    /// Construct a `Remote` error from the wire fields.
    pub fn remote(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Option<serde_json::Value>,
    ) -> Self {
        Self::Remote {
            code: code.into(),
            message: message.into(),
            details,
        }
    }
}

impl From<saikuro_transport::TransportError> for Error {
    fn from(e: saikuro_transport::TransportError) -> Self {
        Self::Transport(e.to_string())
    }
}
