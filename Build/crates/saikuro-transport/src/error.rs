//! Transport error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("connection refused: {0}")]
    ConnectionRefused(String),

    #[error("connection lost: {0}")]
    ConnectionLost(String),

    #[error("send failed: {0}")]
    SendFailed(String),

    #[error("receive failed: {0}")]
    ReceiveFailed(String),

    #[error("message too large: {size} bytes, limit {limit}")]
    MessageTooLarge { size: usize, limit: usize },

    #[error("framing error: {0}")]
    FramingError(String),

    #[error("transport not supported on this platform")]
    NotSupported,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("msgpack encode error: {0}")]
    MsgpackEncode(#[from] rmp_serde::encode::Error),

    #[error("msgpack decode error: {0}")]
    MsgpackDecode(#[from] rmp_serde::decode::Error),

    #[error("channel closed")]
    ChannelClosed,
}

pub type Result<T> = std::result::Result<T, TransportError>;
