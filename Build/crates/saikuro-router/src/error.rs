//! Router error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("no provider registered for namespace '{0}'")]
    NoProvider(String),

    #[error("provider '{0}' is unavailable")]
    ProviderUnavailable(String),

    #[error("malformed target '{0}': must be 'namespace.function'")]
    MalformedTarget(String),

    #[error("stream '{0}' not found")]
    StreamNotFound(String),

    #[error("channel '{0}' not found")]
    ChannelNotFound(String),

    #[error("stream already closed: '{0}'")]
    StreamClosed(String),

    #[error("channel already closed: '{0}'")]
    ChannelClosed(String),

    #[error("batch dispatch failed at item {index}: {reason}")]
    BatchItemFailed { index: usize, reason: String },

    #[error("send error: {0}")]
    SendError(String),
}

pub type Result<T> = std::result::Result<T, RouterError>;
